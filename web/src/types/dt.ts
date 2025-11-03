export interface UnmappedComponent {
  id: number;
  foundation_id: string;
  component_name: string;
  component_version?: string;
  component_group?: string;
  purl?: string;
  classifier?: string;
  mapping_attempts: number;
  first_seen: string;
  last_seen: string;
  mapping_notes?: string;
  external_references?: unknown;
}

export interface ImportHistory {
  foundation_id: string;
  import_timestamp: string;
  components_total: number;
  components_mapped: number;
  components_unmapped: number;
  projects_registered: number;
  duration_seconds?: number;
  success_rate: number;
}

export interface ImportStats {
  total_unmapped: number;
  total_mapped: number;
  mapping_rate_percent: number;
  by_package_type: Record<string, number>;
  recent_imports: ImportHistory[];
}

export interface UnmappedComponentsResponse {
  components: UnmappedComponent[];
  total_count: number;
}

export interface ComponentMapping {
  id: number;
  foundation_id: string;
  component_identifier: string;
  repository_url: string;
  mapping_type: 'manual' | 'automatic' | 'suggested';
  created_by: string;
  created_at: string;
  updated_at: string;
  notes?: string;
}
