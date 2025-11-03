use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DtComponent {
    pub uuid: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub classifier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "externalReferences")]
    pub external_references: Option<Vec<ExternalReference>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExternalReference {
    #[serde(rename = "type")]
    pub type_: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DtProject {
    pub uuid: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dt_component_deserialization() {
        let json = r#"{
            "uuid": "123e4567-e89b-12d3-a456-426614174000",
            "name": "lodash",
            "version": "4.17.21",
            "group": "npm",
            "purl": "pkg:npm/lodash@4.17.21",
            "classifier": "LIBRARY"
        }"#;
        let comp: DtComponent = serde_json::from_str(json).unwrap();
        assert_eq!(comp.name, "lodash");
        assert_eq!(comp.version, Some("4.17.21".to_string()));
        assert_eq!(comp.classifier, "LIBRARY");
    }

    #[test]
    fn test_dt_project_deserialization() {
        let json = r#"{
            "uuid": "proj-123",
            "name": "my-application",
            "version": "1.0.0"
        }"#;
        let proj: DtProject = serde_json::from_str(json).unwrap();
        assert_eq!(proj.name, "my-application");
    }

    #[test]
    fn test_external_reference_deserialization() {
        let json = r#"{
            "type": "vcs",
            "url": "https://github.com/lodash/lodash"
        }"#;
        let ref_: ExternalReference = serde_json::from_str(json).unwrap();
        assert_eq!(ref_.type_, "vcs");
        assert_eq!(ref_.url, "https://github.com/lodash/lodash");
    }
}
