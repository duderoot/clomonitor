import { ImportStats, UnmappedComponent } from '../types/dt';

interface FetchOptions {
  method: 'POST' | 'GET' | 'PUT' | 'DELETE' | 'HEAD';
  headers?: {
    [key: string]: string;
  };
  body?: string;
}

interface APIFetchProps {
  url: string;
  opts?: FetchOptions;
  headers?: string[];
}

class DT_API_CLASS {
  private API_BASE_URL = '/api/dt';

  private async apiFetch(props: APIFetchProps) {
    const options: FetchOptions | Record<string, unknown> = props.opts || {};

    return fetch(props.url, options)
      .then(this.handleErrors)
      .then((res) => this.handleContent(res))
      .catch((error) => Promise.reject(error));
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private async handleErrors(res: any) {
    if (!res.ok) {
      let errorMessage = 'An error occurred';
      try {
        const text = await res.json();
        errorMessage = text.message || errorMessage;
      } catch {
        // Use default error message
      }
      throw new Error(errorMessage);
    }
    return res;
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private async handleContent(res: any) {
    const contentType = res.headers.get('Content-Type');
    if (contentType && contentType.includes('application/json')) {
      return await res.json();
    }
    return res;
  }

  public async getUnmappedComponents(params: {
    foundation_id?: string;
    limit?: number;
    offset?: number;
    search?: string;
  }): Promise<{ components: UnmappedComponent[]; total_count: number }> {
    const queryParams = new URLSearchParams();

    if (params.foundation_id) {
      queryParams.append('foundation_id', params.foundation_id);
    }
    if (params.limit !== undefined) {
      queryParams.append('limit', params.limit.toString());
    }
    if (params.offset !== undefined) {
      queryParams.append('offset', params.offset.toString());
    }
    if (params.search) {
      queryParams.append('search', params.search);
    }

    const queryString = queryParams.toString();
    const url = `${this.API_BASE_URL}/unmapped${queryString ? `?${queryString}` : ''}`;

    return this.apiFetch({
      url,
      opts: {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      },
    });
  }

  public async getImportStats(params: {
    foundation_id?: string;
    date_from?: string;
    date_to?: string;
  }): Promise<ImportStats> {
    const queryParams = new URLSearchParams();

    if (params.foundation_id) {
      queryParams.append('foundation_id', params.foundation_id);
    }
    if (params.date_from) {
      queryParams.append('date_from', params.date_from);
    }
    if (params.date_to) {
      queryParams.append('date_to', params.date_to);
    }

    const queryString = queryParams.toString();
    const url = `${this.API_BASE_URL}/unmapped/stats${queryString ? `?${queryString}` : ''}`;

    return this.apiFetch({
      url,
      opts: {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      },
    });
  }
}

const DT_API = new DT_API_CLASS();
export default DT_API;
