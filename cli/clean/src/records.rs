use dx_setup::{setup_hex, GenerationId, SetupPair};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupRecordView {
    pub hex: String,
    pub environment_hex: String,
    pub generated_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecordProblem {
    #[error("malformed digest {value:?}: want 64-character lowercase hex")]
    MalformedDigest { value: String },
    #[error("spoofed record {record:?}: links resolve to {pair:?}")]
    Spoofed { record: String, pair: String },
}

fn validated_generation_id(hex: &str) -> Result<GenerationId, RecordProblem> {
    GenerationId::new(hex).map_err(|_| RecordProblem::MalformedDigest {
        value: hex.to_owned(),
    })
}

pub fn validate_record(
    hex: &str,
    environment_hex: &str,
    generated_hex: &str,
) -> Result<SetupRecordView, RecordProblem> {
    let environment = validated_generation_id(environment_hex)?;
    let generated = validated_generation_id(generated_hex)?;
    validated_generation_id(hex)?;
    let pair = SetupPair {
        environment,
        generated,
    };
    let want = setup_hex(&pair);
    if want != hex {
        return Err(RecordProblem::Spoofed {
            record: hex.to_owned(),
            pair: want,
        });
    }
    Ok(SetupRecordView {
        hex: hex.to_owned(),
        environment_hex: environment_hex.to_owned(),
        generated_hex: generated_hex.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(tag: char) -> String {
        tag.to_string().repeat(64)
    }

    fn pair_env_gen(env: char, gen: char) -> (String, String, String) {
        let environment = GenerationId::new(&digest(env)).expect("env digest");
        let generated = GenerationId::new(&digest(gen)).expect("gen digest");
        let pair = SetupPair {
            environment,
            generated,
        };
        (setup_hex(&pair), digest(env), digest(gen))
    }

    #[test]
    fn record_validation_pins_pair_identity() {
        let (hex, env_hex, gen_hex) = pair_env_gen('1', '2');
        let view = validate_record(&hex, &env_hex, &gen_hex).expect("valid");
        assert_eq!(view.hex, hex);
        assert_eq!(view.environment_hex, env_hex);
        assert_eq!(view.generated_hex, gen_hex);
    }

    #[test]
    fn malformed_digests_are_refused() {
        assert!(matches!(
            validate_record("not-a-digest", &digest('1'), &digest('2')),
            Err(RecordProblem::MalformedDigest { .. })
        ));
        assert!(matches!(
            validate_record(&digest('a'), &digest('1'), "SPOOF"),
            Err(RecordProblem::MalformedDigest { .. })
        ));
    }

    #[test]
    fn spoofed_record_names_are_refused() {
        let (_, env_hex, gen_hex) = pair_env_gen('1', '2');
        let (other_hex, _, _) = pair_env_gen('3', '4');
        let error = validate_record(&other_hex, &env_hex, &gen_hex).unwrap_err();
        assert!(matches!(error, RecordProblem::Spoofed { .. }));
        assert!(!format!("{error}").is_empty());
    }
}
