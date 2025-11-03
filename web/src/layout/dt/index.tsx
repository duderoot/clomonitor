import { Foundation, SubNavbar } from 'clo-ui';
import { isUndefined } from 'lodash';
import { ChangeEvent } from 'react';
import { useSearchParams } from 'react-router-dom';

import { DEFAULT_FOUNDATION, FOUNDATIONS } from '../../data';
import styles from './DTView.module.css';
import Overview from './Overview';
import UnmappedList from './UnmappedList';

enum TabView {
  Overview = 'overview',
  Unmapped = 'unmapped',
}

const FOUNDATION_QUERY = 'foundation';
const TAB_QUERY = 'tab';

const DTView = () => {
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedFoundation = searchParams.get(FOUNDATION_QUERY) || '';
  const activeTab = (searchParams.get(TAB_QUERY) as TabView) || TabView.Overview;

  const handleFoundationChange = (event: ChangeEvent<HTMLSelectElement>) => {
    const value = event.target.value;
    if (value === '') {
      searchParams.delete(FOUNDATION_QUERY);
    } else {
      searchParams.set(FOUNDATION_QUERY, value);
    }
    setSearchParams(searchParams);
  };

  const handleTabChange = (tab: TabView) => {
    searchParams.set(TAB_QUERY, tab);
    setSearchParams(searchParams);
  };

  return (
    <div className="d-flex flex-column flex-grow-1 position-relative">
      <SubNavbar>
        <div className="d-flex flex-column flex-sm-row align-items-center w-100 justify-content-between my-2">
          <div className="d-flex flex-column">
            <div className="h2 text-dark text-center text-md-start">Dependency-Track Import Visibility</div>
            <small className="text-muted text-center text-md-start">
              Monitor component import success and identify mapping issues
            </small>
          </div>

          <div className={styles.selectWrapper}>
            <div className="d-flex flex-column ms-0 ms-sm-3 mt-3 mt-sm-0 px-4 px-sm-0">
              <label className="form-label me-2 mb-0 fw-bold">Foundation:</label>
              <select
                className={`form-select rounded-0 cursorPointer foundation ${styles.select}`}
                value={selectedFoundation}
                onChange={handleFoundationChange}
                aria-label="Foundation options select"
              >
                <option value="">All Foundations</option>
                {Object.keys(FOUNDATIONS).map((f: string) => {
                  const fData = FOUNDATIONS[f as Foundation];
                  if (isUndefined(fData)) return null;
                  return (
                    <option key={`f_${f}`} value={f}>
                      {fData.name}
                    </option>
                  );
                })}
              </select>
            </div>
          </div>
        </div>
      </SubNavbar>

      <main role="main" className="container-lg py-5 position-relative">
        <ul className="nav nav-tabs mb-4">
          <li className="nav-item">
            <button
              className={`nav-link ${activeTab === TabView.Overview ? 'active' : ''}`}
              onClick={() => handleTabChange(TabView.Overview)}
              type="button"
            >
              Overview
            </button>
          </li>
          <li className="nav-item">
            <button
              className={`nav-link ${activeTab === TabView.Unmapped ? 'active' : ''}`}
              onClick={() => handleTabChange(TabView.Unmapped)}
              type="button"
            >
              Unmapped Components
            </button>
          </li>
        </ul>

        {activeTab === TabView.Overview && <Overview selectedFoundation={selectedFoundation} />}
        {activeTab === TabView.Unmapped && <UnmappedList selectedFoundation={selectedFoundation} />}
      </main>
    </div>
  );
};

export default DTView;
