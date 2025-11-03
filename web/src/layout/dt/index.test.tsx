import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { BrowserRouter as Router } from 'react-router-dom';

import DTView from './index';

// Mock the child components since we're just testing the parent integration
jest.mock('./Overview', () => {
  return function MockOverview({ selectedFoundation }: { selectedFoundation?: string }) {
    return <div data-testid="overview-component">Overview {selectedFoundation}</div>;
  };
});

jest.mock('./UnmappedList', () => {
  return function MockUnmappedList({ selectedFoundation }: { selectedFoundation?: string }) {
    return <div data-testid="unmapped-list-component">UnmappedList {selectedFoundation}</div>;
  };
});

describe('DTView', () => {
  it('renders DTView component with title', () => {
    render(
      <Router>
        <DTView />
      </Router>
    );

    expect(screen.getByText(/Dependency-Track Import Visibility/i)).toBeInTheDocument();
  });

  it('renders foundation selector', () => {
    render(
      <Router>
        <DTView />
      </Router>
    );

    const foundationSelect = screen.getByRole('combobox', { name: /Foundation options select/i });
    expect(foundationSelect).toBeInTheDocument();
  });

  it('renders both tabs', () => {
    render(
      <Router>
        <DTView />
      </Router>
    );

    expect(screen.getByRole('button', { name: /Overview/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Unmapped Components/i })).toBeInTheDocument();
  });

  it('shows Overview tab by default', () => {
    render(
      <Router>
        <DTView />
      </Router>
    );

    expect(screen.getByTestId('overview-component')).toBeInTheDocument();
    expect(screen.queryByTestId('unmapped-list-component')).not.toBeInTheDocument();
  });

  it('switches to Unmapped Components tab when clicked', async () => {
    const user = userEvent.setup();
    render(
      <Router>
        <DTView />
      </Router>
    );

    const unmappedTab = screen.getByRole('button', { name: /Unmapped Components/i });
    await user.click(unmappedTab);

    expect(screen.getByTestId('unmapped-list-component')).toBeInTheDocument();
    expect(screen.queryByTestId('overview-component')).not.toBeInTheDocument();
  });

  it('switches back to Overview tab when clicked', async () => {
    const user = userEvent.setup();
    render(
      <Router>
        <DTView />
      </Router>
    );

    // First switch to Unmapped
    const unmappedTab = screen.getByRole('button', { name: /Unmapped Components/i });
    await user.click(unmappedTab);

    // Then switch back to Overview
    const overviewTab = screen.getByRole('button', { name: /Overview/i });
    await user.click(overviewTab);

    expect(screen.getByTestId('overview-component')).toBeInTheDocument();
    expect(screen.queryByTestId('unmapped-list-component')).not.toBeInTheDocument();
  });

  it('changes foundation selection', async () => {
    const user = userEvent.setup();
    render(
      <Router>
        <DTView />
      </Router>
    );

    const foundationSelect = screen.getByRole('combobox', { name: /Foundation options select/i });
    await user.selectOptions(foundationSelect, 'cdf');

    // The URL should update with the foundation query param
    expect(window.location.search).toContain('foundation=cdf');
  });
});
