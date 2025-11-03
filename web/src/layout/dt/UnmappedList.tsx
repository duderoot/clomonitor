import { alertDispatcher, Foundation, Loading, NoData, Pagination, prettifyNumber } from 'clo-ui';
import moment from 'moment';
import { ChangeEvent, useEffect, useState } from 'react';

import DT_API from '../../api/dt';
import { FOUNDATIONS } from '../../data';
import { UnmappedComponent } from '../../types/dt';
import styles from './UnmappedList.module.css';

interface Props {
  selectedFoundation?: string;
}

interface EditFormData {
  repository_url: string;
  mapping_type: 'manual' | 'automatic' | 'suggested';
  notes: string;
  created_by: string;
}

const ITEMS_PER_PAGE = 20;

const UnmappedList = (props: Props) => {
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [components, setComponents] = useState<UnmappedComponent[]>([]);
  const [totalCount, setTotalCount] = useState<number>(0);
  const [currentPage, setCurrentPage] = useState<number>(1);
  const [searchTerm, setSearchTerm] = useState<string>('');
  const [searchValue, setSearchValue] = useState<string>('');

  // Phase 2: Inline editing state
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editForm, setEditForm] = useState<EditFormData>({
    repository_url: '',
    mapping_type: 'manual',
    notes: '',
    created_by: '',
  });
  const [operationLoading, setOperationLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  useEffect(() => {
    async function fetchComponents() {
      try {
        setIsLoading(true);
        const data = await DT_API.getUnmappedComponents({
          foundation_id: props.selectedFoundation || undefined,
          limit: ITEMS_PER_PAGE,
          offset: (currentPage - 1) * ITEMS_PER_PAGE,
          search: searchTerm,
        });
        setComponents(data.components);
        setTotalCount(data.total_count);
        setIsLoading(false);
      } catch {
        setIsLoading(false);
        alertDispatcher.postAlert({
          type: 'danger',
          message: 'An error occurred loading unmapped components. Please try again later.',
        });
      }
    }

    fetchComponents();
  }, [props.selectedFoundation, currentPage, searchTerm]);

  const refreshData = async () => {
    try {
      const data = await DT_API.getUnmappedComponents({
        foundation_id: props.selectedFoundation || undefined,
        limit: ITEMS_PER_PAGE,
        offset: (currentPage - 1) * ITEMS_PER_PAGE,
        search: searchTerm,
      });
      setComponents(data.components);
      setTotalCount(data.total_count);
    } catch {
      alertDispatcher.postAlert({
        type: 'danger',
        message: 'An error occurred refreshing the component list.',
      });
    }
  };

  const handleSearchChange = (e: ChangeEvent<HTMLInputElement>) => {
    setSearchValue(e.target.value);
  };

  const handleSearchSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setSearchTerm(searchValue);
    setCurrentPage(1);
  };

  const handleClearSearch = () => {
    setSearchValue('');
    setSearchTerm('');
    setCurrentPage(1);
  };

  // Phase 2: Inline editing handlers
  const handleStartEdit = (component: UnmappedComponent) => {
    setEditingId(component.id);
    setEditForm({
      repository_url: '',
      mapping_type: 'manual',
      notes: '',
      created_by: '',
    });
    setError(null);
    setSuccessMessage(null);
  };

  const handleSaveMapping = async (component: UnmappedComponent) => {
    // Validation
    if (!editForm.repository_url.trim()) {
      setError('Repository URL is required');
      return;
    }

    if (!editForm.created_by.trim()) {
      setError('Created By email is required');
      return;
    }

    // Email validation
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (!emailRegex.test(editForm.created_by)) {
      setError('Please enter a valid email address');
      return;
    }

    // URL validation
    try {
      new URL(editForm.repository_url);
    } catch {
      setError('Please enter a valid repository URL');
      return;
    }

    setOperationLoading(true);
    setError(null);
    setSuccessMessage(null);

    try {
      if (!component.purl) {
        setError('Component missing PURL identifier');
        setOperationLoading(false);
        return;
      }

      await DT_API.createComponentMapping({
        foundation_id: component.foundation_id,
        component_identifier: component.purl,
        repository_url: editForm.repository_url,
        mapping_type: editForm.mapping_type,
        created_by: editForm.created_by,
        notes: editForm.notes || undefined,
      });

      setSuccessMessage(`Mapping created successfully for ${component.component_name}`);

      await refreshData();

      setEditingId(null);
      setEditForm({
        repository_url: '',
        mapping_type: 'manual',
        notes: '',
        created_by: '',
      });

      setTimeout(() => setSuccessMessage(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create mapping');
    } finally {
      setOperationLoading(false);
    }
  };

  const handleIgnoreComponent = async (component: UnmappedComponent) => {
    const confirmMessage = `Are you sure you want to ignore "${component.component_name}"?\n\nThis will hide it from the unmapped components list.`;

    if (!window.confirm(confirmMessage)) {
      return;
    }

    const userEmail = prompt('Enter your email address:');
    if (!userEmail) {
      return;
    }

    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (!emailRegex.test(userEmail)) {
      setError('Please enter a valid email address');
      return;
    }

    setOperationLoading(true);
    setError(null);
    setSuccessMessage(null);

    try {
      await DT_API.ignoreComponent(component.id, {
        ignored: true,
        ignored_by: userEmail,
        notes: 'Marked as ignored from UI',
      });

      setSuccessMessage(`Component "${component.component_name}" has been ignored`);
      await refreshData();

      setTimeout(() => setSuccessMessage(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to ignore component');
    } finally {
      setOperationLoading(false);
    }
  };

  const handleCancelEdit = () => {
    setEditingId(null);
    setEditForm({
      repository_url: '',
      mapping_type: 'manual',
      notes: '',
      created_by: '',
    });
    setError(null);
  };

  const totalPages = Math.ceil(totalCount / ITEMS_PER_PAGE);

  return (
    <div className="py-4" data-testid="unmapped-list">
      {successMessage && (
        <div className="alert alert-success alert-dismissible fade show" role="alert" data-testid="success-message">
          <i className="bi bi-check-circle me-2"></i>
          {successMessage}
          <button
            type="button"
            className="btn-close"
            onClick={() => setSuccessMessage(null)}
            aria-label="Close"
          ></button>
        </div>
      )}

      {error && !editingId && (
        <div className="alert alert-danger alert-dismissible fade show" role="alert" data-testid="error-message">
          <i className="bi bi-exclamation-triangle me-2"></i>
          {error}
          <button
            type="button"
            className="btn-close"
            onClick={() => setError(null)}
            aria-label="Close"
          ></button>
        </div>
      )}

      <div className="d-flex flex-column flex-md-row justify-content-between align-items-start align-items-md-center mb-4">
        <div className="mb-3 mb-md-0">
          <h5 className="mb-1">Unmapped Components</h5>
          <small className="text-muted">
            {totalCount > 0 ? `Showing ${prettifyNumber(totalCount)} component${totalCount !== 1 ? 's' : ''}` : ''}
          </small>
        </div>

        <form onSubmit={handleSearchSubmit} className="d-flex gap-2">
          <input
            type="text"
            className="form-control form-control-sm"
            placeholder="Search by name, purl, or group..."
            value={searchValue}
            onChange={handleSearchChange}
            style={{ width: '300px' }}
            data-testid="component-search"
          />
          <button type="submit" className="btn btn-sm btn-primary">
            Search
          </button>
          {searchTerm && (
            <button type="button" className="btn btn-sm btn-secondary" onClick={handleClearSearch}>
              Clear
            </button>
          )}
        </form>
      </div>

      {isLoading && (
        <div className="position-relative" style={{ minHeight: '300px' }}>
          <Loading />
        </div>
      )}

      {!isLoading && components.length === 0 && (
        <NoData>
          <>
            <div className="mb-4 h5">No unmapped components found</div>
            {searchTerm && <p className="mb-0">Try adjusting your search criteria.</p>}
          </>
        </NoData>
      )}

      {!isLoading && components.length > 0 && (
        <>
          <div className="card rounded-0">
            <div className="card-body p-0">
              <div className="table-responsive">
                <table className="table table-sm table-hover mb-0">
                  <thead className="table-light">
                    <tr>
                      <th style={{ width: '18%' }}>Component Name</th>
                      <th style={{ width: '8%' }}>Version</th>
                      <th style={{ width: '12%' }}>Group</th>
                      <th style={{ width: '22%' }}>Package URL</th>
                      <th style={{ width: '6%' }} className="text-center">Attempts</th>
                      <th style={{ width: '10%' }}>Last Seen</th>
                      <th style={{ width: '8%' }}>Foundation</th>
                      <th style={{ width: '16%' }} className="text-end">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {components.map((component: UnmappedComponent) => {
                      const foundationData = FOUNDATIONS[component.foundation_id as Foundation];
                      const componentIdentifier = component.purl || `component-${component.id}`;

                      return (
                        <>
                          <tr key={component.id} data-testid={`component-row-${componentIdentifier}`}>
                            <td>
                              <div className="d-flex flex-column">
                                <span className="fw-semibold">{component.component_name}</span>
                                {component.mapping_notes && typeof component.mapping_notes === 'string' && component.mapping_notes.trim() !== '' && (
                                  <small className="text-danger" title={component.mapping_notes}>
                                    {component.mapping_notes.substring(0, 50)}
                                    {component.mapping_notes.length > 50 ? '...' : ''}
                                  </small>
                                )}
                              </div>
                            </td>
                            <td>
                              <code className="small">{component.component_version || '-'}</code>
                            </td>
                            <td>
                              <small>{component.component_group || '-'}</small>
                            </td>
                            <td>
                              {component.purl ? (
                                <code className="small text-break" style={{ fontSize: '0.75rem' }}>
                                  {component.purl}
                                </code>
                              ) : (
                                <span className="text-muted">-</span>
                              )}
                            </td>
                            <td className="text-center">
                              <span
                                className={`badge ${component.mapping_attempts > 3 ? 'bg-danger' : component.mapping_attempts > 1 ? 'bg-warning' : 'bg-secondary'}`}
                              >
                                {component.mapping_attempts}
                              </span>
                            </td>
                            <td>
                              <small>{moment(component.last_seen).format('YYYY-MM-DD HH:mm')}</small>
                            </td>
                            <td>
                              <span className="badge bg-info" style={{ fontSize: '0.7rem' }}>
                                {foundationData?.name || component.foundation_id}
                              </span>
                            </td>
                            <td className="text-end">
                              {editingId === component.id ? (
                                <div className="btn-group btn-group-sm">
                                  <button
                                    className="btn btn-success"
                                    onClick={() => handleSaveMapping(component)}
                                    disabled={operationLoading}
                                    data-testid="save-mapping-btn"
                                    title="Save mapping"
                                  >
                                    {operationLoading ? (
                                      <>
                                        <span className="spinner-border spinner-border-sm me-1" role="status" aria-hidden="true"></span>
                                        Saving...
                                      </>
                                    ) : (
                                      <>
                                        <i className="bi bi-check-lg me-1"></i>
                                        Save
                                      </>
                                    )}
                                  </button>
                                  <button
                                    className="btn btn-secondary"
                                    onClick={handleCancelEdit}
                                    disabled={operationLoading}
                                    data-testid="cancel-mapping-btn"
                                    title="Cancel"
                                  >
                                    <i className="bi bi-x-lg me-1"></i>
                                    Cancel
                                  </button>
                                </div>
                              ) : (
                                <div className="btn-group btn-group-sm">
                                  <button
                                    className="btn btn-primary"
                                    onClick={() => handleStartEdit(component)}
                                    disabled={operationLoading}
                                    data-testid={`add-mapping-btn-${componentIdentifier}`}
                                    title="Add repository mapping"
                                  >
                                    <i className="bi bi-link-45deg me-1"></i>
                                    Add Mapping
                                  </button>
                                  <button
                                    className="btn btn-warning"
                                    onClick={() => handleIgnoreComponent(component)}
                                    disabled={operationLoading}
                                    data-testid={`ignore-btn-${componentIdentifier}`}
                                    title="Ignore this component"
                                  >
                                    <i className="bi bi-eye-slash me-1"></i>
                                    Ignore
                                  </button>
                                </div>
                              )}
                            </td>
                          </tr>
                          {editingId === component.id && (
                            <tr data-testid={`mapping-form-${componentIdentifier}`}>
                              <td colSpan={8}>
                                <div className="p-3 bg-light border rounded">
                                  <h6 className="mb-3">
                                    <i className="bi bi-pencil-square me-2"></i>
                                    Create Mapping for <strong>{component.component_name}</strong>
                                  </h6>

                                  {error && (
                                    <div className="alert alert-danger alert-sm" data-testid="error-message">
                                      <i className="bi bi-exclamation-triangle me-2"></i>
                                      {error}
                                    </div>
                                  )}

                                  <div className="row g-3">
                                    <div className="col-md-6">
                                      <label className="form-label fw-semibold">
                                        Repository URL <span className="text-danger">*</span>
                                      </label>
                                      <input
                                        type="url"
                                        className="form-control"
                                        value={editForm.repository_url}
                                        onChange={(e) => setEditForm({ ...editForm, repository_url: e.target.value })}
                                        placeholder="https://github.com/org/repo"
                                        required
                                        disabled={operationLoading}
                                        data-testid="repository-url-input"
                                      />
                                      <div className="form-text">
                                        Enter the GitHub repository URL for this component
                                      </div>
                                    </div>

                                    <div className="col-md-3">
                                      <label className="form-label fw-semibold">Mapping Type</label>
                                      <select
                                        className="form-select"
                                        value={editForm.mapping_type}
                                        onChange={(e) => setEditForm({ ...editForm, mapping_type: e.target.value as EditFormData['mapping_type'] })}
                                        disabled={operationLoading}
                                        data-testid="mapping-type-select"
                                      >
                                        <option value="manual">Manual</option>
                                        <option value="automatic">Automatic</option>
                                        <option value="suggested">Suggested</option>
                                      </select>
                                      <div className="form-text">
                                        How this mapping was created
                                      </div>
                                    </div>

                                    <div className="col-md-3">
                                      <label className="form-label fw-semibold">
                                        Created By <span className="text-danger">*</span>
                                      </label>
                                      <input
                                        type="email"
                                        className="form-control"
                                        value={editForm.created_by}
                                        onChange={(e) => setEditForm({ ...editForm, created_by: e.target.value })}
                                        placeholder="user@example.com"
                                        required
                                        disabled={operationLoading}
                                        data-testid="created-by-input"
                                      />
                                      <div className="form-text">
                                        Your email address
                                      </div>
                                    </div>

                                    <div className="col-12">
                                      <label className="form-label fw-semibold">
                                        Notes <span className="text-muted">(Optional)</span>
                                      </label>
                                      <textarea
                                        className="form-control"
                                        value={editForm.notes}
                                        onChange={(e) => setEditForm({ ...editForm, notes: e.target.value })}
                                        rows={2}
                                        placeholder="Optional notes about this mapping..."
                                        disabled={operationLoading}
                                        data-testid="notes-textarea"
                                      />
                                    </div>
                                  </div>

                                  <div className="mt-3 p-2 bg-white border rounded">
                                    <small className="text-muted">
                                      <i className="bi bi-info-circle me-1"></i>
                                      <strong>Component PURL:</strong> <code>{component.purl || 'N/A'}</code>
                                    </small>
                                  </div>
                                </div>
                              </td>
                            </tr>
                          )}
                        </>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </div>
          </div>

          {totalPages > 1 && (
            <div className="mt-4 d-flex justify-content-center" data-testid="pagination">
              <Pagination
                limit={ITEMS_PER_PAGE}
                offset={(currentPage - 1) * ITEMS_PER_PAGE}
                total={totalCount}
                active={currentPage}
                className={styles.pagination}
                onChange={(pageNumber: number) => setCurrentPage(pageNumber)}
              />
              <span className="ms-3 align-self-center" data-testid="page-info">
                Page {currentPage} of {totalPages}
              </span>
            </div>
          )}
        </>
      )}
    </div>
  );
};

export default UnmappedList;
