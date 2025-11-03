import { alertDispatcher, Foundation, Loading, NoData, prettifyNumber } from 'clo-ui';
import { isUndefined } from 'lodash';
import moment from 'moment';
import { useEffect, useState } from 'react';

import DT_API from '../../api/dt';
import { FOUNDATIONS } from '../../data';
import { ImportHistory, ImportStats } from '../../types/dt';

interface Props {
  selectedFoundation?: string;
}

const Overview = (props: Props) => {
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [stats, setStats] = useState<ImportStats | null>(null);

  useEffect(() => {
    async function fetchStats() {
      try {
        setIsLoading(true);
        const data = await DT_API.getImportStats({
          foundation_id: props.selectedFoundation,
        });
        setStats(data);
        setIsLoading(false);
      } catch {
        setIsLoading(false);
        alertDispatcher.postAlert({
          type: 'danger',
          message: 'An error occurred loading DT import statistics. Please try again later.',
        });
      }
    }

    fetchStats();
  }, [props.selectedFoundation]);

  if (isLoading) {
    return (
      <div className="position-relative" style={{ minHeight: '400px' }}>
        <Loading />
      </div>
    );
  }

  if (!stats) {
    return (
      <NoData>
        <div className="mb-4 h4">No import statistics available</div>
        <p className="mb-0">Please try again later.</p>
      </NoData>
    );
  }

  const mappingRateColor = (rate: number): string => {
    if (rate >= 90) return 'text-success';
    if (rate >= 70) return 'text-warning';
    return 'text-danger';
  };

  return (
    <div className="py-4">
      <div className="row g-4 mb-5">
        <div className="col-12 col-md-4">
          <div className="card rounded-0 h-100">
            <div className="card-body text-center">
              <div className="text-muted text-uppercase small mb-2">Total Components</div>
              <div className="h2 mb-2">{prettifyNumber(stats.total_mapped + stats.total_unmapped)}</div>
              <div className="small">
                <span className="text-success">{prettifyNumber(stats.total_mapped)} mapped</span>
                <span className="mx-2">|</span>
                <span className="text-danger">{prettifyNumber(stats.total_unmapped)} unmapped</span>
              </div>
            </div>
          </div>
        </div>

        <div className="col-12 col-md-4">
          <div className="card rounded-0 h-100">
            <div className="card-body text-center">
              <div className="text-muted text-uppercase small mb-2">Mapping Success Rate</div>
              <div className={`h2 mb-2 ${mappingRateColor(stats.mapping_rate_percent || 0)}`}>
                {stats.mapping_rate_percent !== undefined ? stats.mapping_rate_percent.toFixed(1) : '0.0'}%
              </div>
              <div className="small text-muted">Overall import success rate</div>
            </div>
          </div>
        </div>

        <div className="col-12 col-md-4">
          <div className="card rounded-0 h-100">
            <div className="card-body text-center">
              <div className="text-muted text-uppercase small mb-2">Package Types</div>
              <div className="h2 mb-2">{stats.by_package_type ? Object.keys(stats.by_package_type).length : 0}</div>
              <div className="small text-muted">Different package ecosystems</div>
            </div>
          </div>
        </div>
      </div>

      {!isUndefined(stats.by_package_type) && Object.keys(stats.by_package_type).length > 0 && (
        <div className="mb-5">
          <h5 className="mb-3">Components by Package Type</h5>
          <div className="card rounded-0">
            <div className="card-body">
              <div className="table-responsive">
                <table className="table table-sm table-striped mb-0">
                  <thead>
                    <tr>
                      <th>Package Type</th>
                      <th className="text-end">Count</th>
                    </tr>
                  </thead>
                  <tbody>
                    {Object.entries(stats.by_package_type)
                      .sort(([, a], [, b]) => b - a)
                      .map(([type, count]) => (
                        <tr key={type}>
                          <td>
                            <code>{type || 'unknown'}</code>
                          </td>
                          <td className="text-end">{prettifyNumber(count)}</td>
                        </tr>
                      ))}
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </div>
      )}

      {!isUndefined(stats.recent_imports) && stats.recent_imports.length > 0 && (
        <div>
          <h5 className="mb-3">Recent Import Runs</h5>
          <div className="card rounded-0">
            <div className="card-body">
              <div className="table-responsive">
                <table className="table table-sm table-striped mb-0">
                  <thead>
                    <tr>
                      <th>Foundation</th>
                      <th>Timestamp</th>
                      <th className="text-end">Total</th>
                      <th className="text-end">Mapped</th>
                      <th className="text-end">Unmapped</th>
                      <th className="text-end">Projects</th>
                      <th className="text-end">Success Rate</th>
                      <th className="text-end">Duration</th>
                    </tr>
                  </thead>
                  <tbody>
                    {stats.recent_imports.map((importRun: ImportHistory, idx: number) => {
                      const foundationData = FOUNDATIONS[importRun.foundation_id as Foundation];
                      return (
                        <tr key={`import-${idx}`}>
                          <td>
                            <span className="badge bg-secondary">{foundationData?.name || importRun.foundation_id}</span>
                          </td>
                          <td>
                            <small>{moment(importRun.import_timestamp).format('YYYY-MM-DD HH:mm:ss')}</small>
                          </td>
                          <td className="text-end">{prettifyNumber(importRun.components_total)}</td>
                          <td className="text-end text-success">{prettifyNumber(importRun.components_mapped)}</td>
                          <td className="text-end text-danger">{prettifyNumber(importRun.components_unmapped)}</td>
                          <td className="text-end">{prettifyNumber(importRun.projects_registered)}</td>
                          <td className={`text-end ${mappingRateColor(importRun.success_rate || 0)}`}>
                            {importRun.success_rate !== undefined ? importRun.success_rate.toFixed(1) : '0.0'}%
                          </td>
                          <td className="text-end">
                            <small>
                              {importRun.duration_seconds !== undefined && importRun.duration_seconds !== null ? `${importRun.duration_seconds.toFixed(1)}s` : '-'}
                            </small>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default Overview;
