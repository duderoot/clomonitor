import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

import App from './App';

// Mock the API calls
jest.mock('./api/dt', () => ({
  __esModule: true,
  default: {
    getImportStats: jest.fn().mockResolvedValue({
      total_unmapped: 0,
      total_mapped: 0,
      mapping_rate_percent: 0,
      by_package_type: {},
      recent_imports: [],
    }),
    getUnmappedComponents: jest.fn().mockResolvedValue({
      components: [],
      total_count: 0,
    }),
  },
}));

// Mock API for other routes
jest.mock('./api', () => ({
  __esModule: true,
  default: {
    getProjects: jest.fn().mockResolvedValue({
      projects: [],
      pagination: { total_count: 0, limit: 20, offset: 0 },
    }),
    getStats: jest.fn().mockResolvedValue({}),
  },
}));

describe('App routing', () => {
  it('renders DT Visibility route', async () => {
    render(
      <MemoryRouter initialEntries={['/dt-visibility']}>
        <App />
      </MemoryRouter>
    );

    // Should render the DT Visibility page heading
    expect(await screen.findByText(/Dependency-Track Import Visibility/i)).toBeInTheDocument();
  });

  it('renders Overview tab by default in DT Visibility', async () => {
    render(
      <MemoryRouter initialEntries={['/dt-visibility']}>
        <App />
      </MemoryRouter>
    );

    const overviewButton = await screen.findByRole('button', { name: /Overview/i });
    expect(overviewButton).toBeInTheDocument();
    expect(overviewButton).toHaveClass('active');
  });

  it('renders Unmapped Components tab in DT Visibility', async () => {
    render(
      <MemoryRouter initialEntries={['/dt-visibility']}>
        <App />
      </MemoryRouter>
    );

    const unmappedButton = await screen.findByRole('button', { name: /Unmapped Components/i });
    expect(unmappedButton).toBeInTheDocument();
  });
});
