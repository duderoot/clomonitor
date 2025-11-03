import { render, screen } from '@testing-library/react';
import { BrowserRouter } from 'react-router-dom';

import DTView from './index';

// Mock the API module to prevent actual API calls during tests
jest.mock('../../api/dt', () => ({
  __esModule: true,
  default: {
    getUnmappedComponents: jest.fn().mockResolvedValue({
      components: [],
      total_count: 0,
    }),
    getImportStats: jest.fn().mockResolvedValue({
      total_unmapped: 0,
      total_mapped: 0,
      mapping_rate_percent: 0,
      by_package_type: {},
      recent_imports: [],
    }),
  },
}));

describe('DTView Component', () => {
  test('renders the DT Visibility page header', () => {
    render(
      <BrowserRouter>
        <DTView />
      </BrowserRouter>
    );

    expect(screen.getByText(/Dependency-Track Import Visibility/i)).toBeInTheDocument();
  });

  test('renders the foundation selector', () => {
    render(
      <BrowserRouter>
        <DTView />
      </BrowserRouter>
    );

    expect(screen.getByLabelText(/Foundation:/i)).toBeInTheDocument();
  });

  test('renders both tab buttons', () => {
    render(
      <BrowserRouter>
        <DTView />
      </BrowserRouter>
    );

    expect(screen.getByRole('button', { name: /Overview/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Unmapped Components/i })).toBeInTheDocument();
  });

  test('shows the Overview tab by default', () => {
    render(
      <BrowserRouter>
        <DTView />
      </BrowserRouter>
    );

    const overviewButton = screen.getByRole('button', { name: /Overview/i });
    expect(overviewButton).toHaveClass('active');
  });
});
