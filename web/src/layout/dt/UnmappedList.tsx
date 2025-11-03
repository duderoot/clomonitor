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

const ITEMS_PER_PAGE = 20;

const UnmappedList = (props: Props) => {
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [components, setComponents] = useState<UnmappedComponent[]>([]);
  const [totalCount, setTotalCount] = useState<number>(0);
  const [currentPage, setCurrentPage] = useState<number>(1);
  const [searchTerm, setSearchTerm] = useState<string>('');
  const [searchValue, setSearchValue] = useState<string>('');

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

  const totalPages = Math.ceil(totalCount / ITEMS_PER_PAGE);

  return (
    <div className="py-4">
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
                      <th style={{ width: '5%' }}>ID</th>
                      <th style={{ width: '20%' }}>Component Name</th>
                      <th style={{ width: '10%' }}>Version</th>
                      <th style={{ width: '15%' }}>Group</th>
                      <th style={{ width: '25%' }}>Package URL</th>
                      <th style={{ width: '8%' }} className="text-center">
                        Attempts
                      </th>
                      <th style={{ width: '12%' }}>Last Seen</th>
                      <th style={{ width: '5%' }}>Foundation</th>
                    </tr>
                  </thead>
                  <tbody>
                    {components.map((component: UnmappedComponent) => {
                      const foundationData = FOUNDATIONS[component.foundation_id as Foundation];
                      return (
                        <tr key={component.id}>
                          <td>
                            <small className="text-muted">{component.id}</small>
                          </td>
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
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </div>
          </div>

          {totalPages > 1 && (
            <div className="mt-4 d-flex justify-content-center">
              <Pagination
                limit={ITEMS_PER_PAGE}
                offset={(currentPage - 1) * ITEMS_PER_PAGE}
                total={totalCount}
                active={currentPage}
                className={styles.pagination}
                onChange={(pageNumber: number) => setCurrentPage(pageNumber)}
              />
            </div>
          )}
        </>
      )}
    </div>
  );
};

export default UnmappedList;
