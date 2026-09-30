use super::scenario::{
    parse_and_validate_manifest_v1, ScenarioBackendV1, ScenarioInputSpecV1,
    ScenarioManifestV1, ScenarioProfilingPolicyV1,
    ScenarioVisualizationProfileV1,
};
use super::observability::RunObservationLevel;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub(crate) const SCENARIO_PRESET_REGISTRY_SCHEMA_VERSION: u32 = 1;

const PRESET_SOURCES: &[&str] = &[
    include_str!("../scenarios/mux1-lifecycle-v1.json"),
    include_str!("../scenarios/xor32-lifecycle-v1.json"),
    include_str!("../scenarios/add32-lifecycle-v1.json"),
    include_str!("../scenarios/shl32-lifecycle-v1.json"),
    include_str!("../scenarios/mul32-lifecycle-v1.json"),
    include_str!("../scenarios/radix-memory8-lifecycle-v1.json"),
    include_str!("../scenarios/and32-lifecycle-v1.json"),
    include_str!("../scenarios/or32-lifecycle-v1.json"),
    include_str!("../scenarios/not32-lifecycle-v1.json"),
    include_str!("../scenarios/test32-lifecycle-v1.json"),
    include_str!("../scenarios/adc32-lifecycle-v1.json"),
    include_str!("../scenarios/sub32-lifecycle-v1.json"),
    include_str!("../scenarios/sbb32-lifecycle-v1.json"),
    include_str!("../scenarios/cmp32-lifecycle-v1.json"),
    include_str!("../scenarios/shr32-lifecycle-v1.json"),
    include_str!("../scenarios/sar32-lifecycle-v1.json"),
    include_str!("../scenarios/rol32-lifecycle-v1.json"),
    include_str!("../scenarios/ror32-lifecycle-v1.json"),
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioPresetSummaryV1 {
    pub(crate) scenario_id: String,
    pub(crate) scenario_version: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) program_profile_id: String,
    pub(crate) family: String,
    pub(crate) program_id: String,
    pub(crate) supported_backends: Vec<ScenarioBackendV1>,
    pub(crate) observation_level: RunObservationLevel,
    pub(crate) profiling_policy: ScenarioProfilingPolicyV1,
    pub(crate) visualization_profile: ScenarioVisualizationProfileV1,
    pub(crate) run_count: u32,
    pub(crate) input_schema: Vec<ScenarioInputSpecV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioPresetRegistryV1 {
    pub(crate) schema_version: u32,
    pub(crate) entries: Vec<ScenarioPresetSummaryV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ScenarioPresetRegistryErrorV1 {
    InvalidPreset {
        index: u32,
        message: String,
    },
    DuplicateIdentity {
        scenario_id: String,
        scenario_version: String,
    },
    DuplicateSource {
        index: u32,
    },
    PresetNotFound {
        scenario_id: String,
        scenario_version: String,
    },
    PresetIndexOutOfRange {
        index: u32,
        count: u32,
    },
}

fn manifest_to_summary(
    manifest: &ScenarioManifestV1,
) -> ScenarioPresetSummaryV1 {
    ScenarioPresetSummaryV1 {
        scenario_id: manifest.scenario_id.clone(),
        scenario_version: manifest.scenario_version.clone(),
        title: manifest.title.clone(),
        description: manifest.description.clone(),
        program_profile_id: manifest.program_profile.profile_id.clone(),
        family: manifest.program_profile.family.clone(),
        program_id: manifest.program_profile.program_id.clone(),
        supported_backends: manifest.supported_backends.clone(),
        observation_level: manifest.observation_level,
        profiling_policy: manifest.profiling_policy,
        visualization_profile: manifest.visualization_profile,
        run_count: manifest.run_sequence.len() as u32,
        input_schema: manifest.input_schema.clone(),
    }
}

fn parse_source(
    index: usize,
    source: &str,
) -> Result<ScenarioManifestV1, ScenarioPresetRegistryErrorV1> {
    parse_and_validate_manifest_v1(source).map_err(|errors| {
        ScenarioPresetRegistryErrorV1::InvalidPreset {
            index: index as u32,
            message: serde_json::to_string(&errors)
                .unwrap_or_else(|_| "manifest validation failed".to_owned()),
        }
    })
}

pub(crate) fn load_preset_registry_v1(
) -> Result<ScenarioPresetRegistryV1, ScenarioPresetRegistryErrorV1> {
    let mut identities = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut entries = Vec::with_capacity(PRESET_SOURCES.len());

    for (index, source) in PRESET_SOURCES.iter().copied().enumerate() {
        if !sources.insert(source.as_bytes()) {
            return Err(ScenarioPresetRegistryErrorV1::DuplicateSource {
                index: index as u32,
            });
        }

        let manifest = parse_source(index, source)?;
        let identity = (
            manifest.scenario_id.clone(),
            manifest.scenario_version.clone(),
        );
        if !identities.insert(identity.clone()) {
            return Err(
                ScenarioPresetRegistryErrorV1::DuplicateIdentity {
                    scenario_id: identity.0,
                    scenario_version: identity.1,
                },
            );
        }

        entries.push(manifest_to_summary(&manifest));
    }

    Ok(ScenarioPresetRegistryV1 {
        schema_version: SCENARIO_PRESET_REGISTRY_SCHEMA_VERSION,
        entries,
    })
}

pub(crate) fn preset_manifest_source_by_index_v1(
    index: u32,
) -> Result<&'static str, ScenarioPresetRegistryErrorV1> {
    PRESET_SOURCES
        .get(index as usize)
        .copied()
        .ok_or(ScenarioPresetRegistryErrorV1::PresetIndexOutOfRange {
            index,
            count: PRESET_SOURCES.len() as u32,
        })
}

pub(crate) fn preset_manifest_source_v1(
    scenario_id: &str,
    scenario_version: &str,
) -> Result<&'static str, ScenarioPresetRegistryErrorV1> {
    for (index, source) in PRESET_SOURCES.iter().copied().enumerate() {
        let manifest = parse_source(index, source)?;
        if manifest.scenario_id == scenario_id
            && manifest.scenario_version == scenario_version
        {
            return Ok(source);
        }
    }

    Err(ScenarioPresetRegistryErrorV1::PresetNotFound {
        scenario_id: scenario_id.to_owned(),
        scenario_version: scenario_version.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_summary_is_derived_from_canonical_mux1_manifest() {
        let registry = load_preset_registry_v1().unwrap();
        assert_eq!(registry.schema_version, 1);
        assert_eq!(registry.entries.len(), 18);

        let source = preset_manifest_source_by_index_v1(0).unwrap();
        let manifest = parse_and_validate_manifest_v1(source).unwrap();
        let summary = &registry.entries[0];

        assert_eq!(summary.scenario_id, manifest.scenario_id);
        assert_eq!(summary.scenario_version, manifest.scenario_version);
        assert_eq!(summary.title, manifest.title);
        assert_eq!(summary.description, manifest.description);
        assert_eq!(
            summary.program_profile_id,
            manifest.program_profile.profile_id
        );
        assert_eq!(summary.family, manifest.program_profile.family);
        assert_eq!(summary.program_id, manifest.program_profile.program_id);
        assert_eq!(summary.supported_backends, manifest.supported_backends);
        assert_eq!(summary.observation_level, manifest.observation_level);
        assert_eq!(summary.profiling_policy, manifest.profiling_policy);
        assert_eq!(
            summary.visualization_profile,
            manifest.visualization_profile
        );
        assert_eq!(summary.run_count, manifest.run_sequence.len() as u32);
        assert_eq!(summary.input_schema, manifest.input_schema);
    }

    #[test]
    fn registry_can_resolve_exact_canonical_manifest_by_identity() {
        let source = preset_manifest_source_v1("mux1-lifecycle", "1.0.0")
            .unwrap();
        assert_eq!(
            source,
            include_str!("../scenarios/mux1-lifecycle-v1.json")
        );
    }

    #[test]
    fn registry_resolves_xor32_canonical_manifest() {
        let source =
            preset_manifest_source_v1("xor32-lifecycle", "1.0.0").unwrap();
        assert_eq!(
            source,
            include_str!("../scenarios/xor32-lifecycle-v1.json")
        );

        let registry = load_preset_registry_v1().unwrap();
        let summary = registry
            .entries
            .iter()
            .find(|entry| entry.scenario_id == "xor32-lifecycle")
            .unwrap();
        assert_eq!(summary.program_profile_id, "a-circuit:logic-xor32");
        assert_eq!(summary.family, "logic-effect");
        assert_eq!(summary.program_id, "xor32");
        assert_eq!(summary.run_count, 4);
    }

    #[test]
    fn registry_resolves_complete_logic32_family() {
        let registry = load_preset_registry_v1().unwrap();
        for (scenario_id, profile_id, program_id, inputs) in [
            ("and32-lifecycle", "a-circuit:logic-and32", "and32", 2usize),
            ("or32-lifecycle", "a-circuit:logic-or32", "or32", 2usize),
            ("xor32-lifecycle", "a-circuit:logic-xor32", "xor32", 2usize),
            ("not32-lifecycle", "a-circuit:logic-not32", "not32", 1usize),
            ("test32-lifecycle", "a-circuit:logic-test32", "test32", 2usize),
        ] {
            let source =
                preset_manifest_source_v1(scenario_id, "1.0.0").unwrap();
            let manifest = parse_and_validate_manifest_v1(source).unwrap();
            assert_eq!(manifest.program_profile.profile_id, profile_id);
            assert_eq!(manifest.program_profile.program_id, program_id);
            assert_eq!(manifest.input_schema.len(), inputs);
            assert_eq!(manifest.run_sequence.len(), 4);

            let summary = registry
                .entries
                .iter()
                .find(|entry| entry.scenario_id == scenario_id)
                .unwrap();
            assert_eq!(summary.program_profile_id, profile_id);
            assert_eq!(summary.family, "logic-effect");
            assert_eq!(summary.program_id, program_id);
            assert_eq!(summary.run_count, 4);
            assert_eq!(summary.input_schema.len(), inputs);
        }
    }

    #[test]
    fn registry_resolves_add32_canonical_manifest() {
        let source =
            preset_manifest_source_v1("add32-lifecycle", "1.0.0").unwrap();
        assert_eq!(
            source,
            include_str!("../scenarios/add32-lifecycle-v1.json")
        );

        let registry = load_preset_registry_v1().unwrap();
        let summary = registry
            .entries
            .iter()
            .find(|entry| entry.scenario_id == "add32-lifecycle")
            .unwrap();
        assert_eq!(
            summary.program_profile_id,
            "a-circuit:arithmetic-add32"
        );
        assert_eq!(summary.family, "arithmetic-effect");
        assert_eq!(summary.program_id, "add32");
        assert_eq!(summary.run_count, 4);
    }

    #[test]
    fn registry_resolves_complete_arithmetic32_family() {
        let registry = load_preset_registry_v1().unwrap();
        for (scenario_id, profile_id, program_id, inputs) in [
            ("add32-lifecycle", "a-circuit:arithmetic-add32", "add32", 2usize),
            ("adc32-lifecycle", "a-circuit:arithmetic-adc32", "adc32", 3usize),
            ("sub32-lifecycle", "a-circuit:arithmetic-sub32", "sub32", 2usize),
            ("sbb32-lifecycle", "a-circuit:arithmetic-sbb32", "sbb32", 3usize),
            ("cmp32-lifecycle", "a-circuit:arithmetic-cmp32", "cmp32", 2usize),
        ] {
            let source =
                preset_manifest_source_v1(scenario_id, "1.0.0").unwrap();
            let manifest = parse_and_validate_manifest_v1(source).unwrap();
            assert_eq!(manifest.program_profile.profile_id, profile_id);
            assert_eq!(manifest.program_profile.program_id, program_id);
            assert_eq!(manifest.input_schema.len(), inputs);
            assert_eq!(manifest.run_sequence.len(), 4);

            let summary = registry
                .entries
                .iter()
                .find(|entry| entry.scenario_id == scenario_id)
                .unwrap();
            assert_eq!(summary.program_profile_id, profile_id);
            assert_eq!(summary.family, "arithmetic-effect");
            assert_eq!(summary.program_id, program_id);
            assert_eq!(summary.run_count, 4);
            assert_eq!(summary.input_schema.len(), inputs);
        }
    }

    #[test]
    fn registry_resolves_complete_shift32_family() {
        let registry = load_preset_registry_v1().unwrap();
        for (scenario_id, profile_id, program_id, source) in [
            (
                "shl32-lifecycle",
                "a-circuit:shift-shl32",
                "shl32",
                include_str!("../scenarios/shl32-lifecycle-v1.json"),
            ),
            (
                "shr32-lifecycle",
                "a-circuit:shift-shr32",
                "shr32",
                include_str!("../scenarios/shr32-lifecycle-v1.json"),
            ),
            (
                "sar32-lifecycle",
                "a-circuit:shift-sar32",
                "sar32",
                include_str!("../scenarios/sar32-lifecycle-v1.json"),
            ),
        ] {
            let resolved =
                preset_manifest_source_v1(scenario_id, "1.0.0").unwrap();
            assert_eq!(resolved, source);

            let manifest =
                parse_and_validate_manifest_v1(resolved).unwrap();
            assert_eq!(manifest.program_profile.profile_id, profile_id);
            assert_eq!(manifest.program_profile.program_id, program_id);
            assert_eq!(manifest.run_sequence.len(), 5);

            let summary = registry
                .entries
                .iter()
                .find(|entry| entry.scenario_id == scenario_id)
                .unwrap();
            assert_eq!(summary.program_profile_id, profile_id);
            assert_eq!(summary.family, "shift-effect");
            assert_eq!(summary.program_id, program_id);
            assert_eq!(summary.run_count, 5);
            assert_eq!(summary.input_schema.len(), 2);
        }

        // Compatibility: the historical 14 entries do not move.
        assert_eq!(registry.entries[3].scenario_id, "shl32-lifecycle");
        assert_eq!(registry.entries[13].scenario_id, "cmp32-lifecycle");
        assert_eq!(registry.entries[14].scenario_id, "shr32-lifecycle");
        assert_eq!(registry.entries[15].scenario_id, "sar32-lifecycle");
    }

    #[test]
    fn registry_resolves_complete_rotate32_family() {
        let registry = load_preset_registry_v1().unwrap();
        for (scenario_id, profile_id, program_id, source) in [
            (
                "rol32-lifecycle",
                "a-circuit:rotate-rol32",
                "rol32",
                include_str!("../scenarios/rol32-lifecycle-v1.json"),
            ),
            (
                "ror32-lifecycle",
                "a-circuit:rotate-ror32",
                "ror32",
                include_str!("../scenarios/ror32-lifecycle-v1.json"),
            ),
        ] {
            let resolved =
                preset_manifest_source_v1(scenario_id, "1.0.0").unwrap();
            assert_eq!(resolved, source);
            let manifest =
                parse_and_validate_manifest_v1(resolved).unwrap();
            assert_eq!(manifest.program_profile.profile_id, profile_id);
            assert_eq!(manifest.program_profile.program_id, program_id);
            assert_eq!(manifest.run_sequence.len(), 6);

            let summary = registry
                .entries
                .iter()
                .find(|entry| entry.scenario_id == scenario_id)
                .unwrap();
            assert_eq!(summary.program_profile_id, profile_id);
            assert_eq!(summary.family, "rotate-effect");
            assert_eq!(summary.program_id, program_id);
            assert_eq!(summary.run_count, 6);
            assert_eq!(summary.input_schema.len(), 2);
        }

        assert_eq!(registry.entries[15].scenario_id, "sar32-lifecycle");
        assert_eq!(registry.entries[16].scenario_id, "rol32-lifecycle");
        assert_eq!(registry.entries[17].scenario_id, "ror32-lifecycle");
    }

    #[test]
    fn registry_resolves_mul32_canonical_manifest() {
        let source =
            preset_manifest_source_v1("mul32-lifecycle", "1.0.0").unwrap();
        assert_eq!(
            source,
            include_str!("../scenarios/mul32-lifecycle-v1.json")
        );

        let registry = load_preset_registry_v1().unwrap();
        let summary = registry
            .entries
            .iter()
            .find(|entry| entry.scenario_id == "mul32-lifecycle")
            .unwrap();
        assert_eq!(summary.program_profile_id, "a-circuit:mul32");
        assert_eq!(summary.family, "mul");
        assert_eq!(summary.program_id, "mul32");
        assert_eq!(summary.run_count, 4);
    }

    #[test]
    fn registry_resolves_m6a_radix_memory_canonical_manifest() {
        let source = preset_manifest_source_v1(
            "radix-memory8-lifecycle",
            "1.0.0",
        )
        .unwrap();
        assert_eq!(
            source,
            include_str!("../scenarios/radix-memory8-lifecycle-v1.json")
        );

        let registry = load_preset_registry_v1().unwrap();
        let summary = registry
            .entries
            .iter()
            .find(|entry| {
                entry.scenario_id == "radix-memory8-lifecycle"
            })
            .unwrap();
        assert_eq!(
            summary.program_profile_id,
            "a-circuit:memory-radix8"
        );
        assert_eq!(summary.family, "memory");
        assert_eq!(summary.program_id, "radix-page8");
        assert_eq!(summary.run_count, 4);
        assert_eq!(summary.observation_level, RunObservationLevel::Trace);
    }

    #[test]
    fn registry_rejects_unknown_identity_and_index() {
        assert!(matches!(
            preset_manifest_source_v1("missing", "1.0.0"),
            Err(ScenarioPresetRegistryErrorV1::PresetNotFound { .. })
        ));
        assert!(matches!(
            preset_manifest_source_by_index_v1(99),
            Err(
                ScenarioPresetRegistryErrorV1::PresetIndexOutOfRange { .. }
            )
        ));
    }
}
