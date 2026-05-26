pub mod spirit_intent {
    use nota_codec::{Decoder, NotaDecode, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topic(pub String);

impl NotaDecode for Topic {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        Ok(Self(<String as NotaDecode>::decode(decoder)?))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topics(pub Vec<Topic>);

impl NotaDecode for Topics {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        Ok(Self(<Vec<Topic> as NotaDecode>::decode(decoder)?))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Description(pub String);

impl NotaDecode for Description {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        Ok(Self(<String as NotaDecode>::decode(decoder)?))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub topics: Topics,
    pub kind: Kind,
    pub description: Description,
    pub magnitude: Magnitude,
}

impl NotaDecode for Entry {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        decoder.expect_positional_record_start("Entry", 4)?;
        let topics = <Topics as NotaDecode>::decode(decoder)?;
        let kind = <Kind as NotaDecode>::decode(decoder)?;
        let description = <Description as NotaDecode>::decode(decoder)?;
        let magnitude = <Magnitude as NotaDecode>::decode(decoder)?;
        decoder.expect_record_end()?;
        Ok(Self {
            topics,
            kind,
            description,
            magnitude,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Decision,
    Principle,
    Correction,
    Clarification,
    Constraint,
}

impl NotaDecode for Kind {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        if decoder.peek_is_record_start()? {
            let variant = decoder.peek_record_head()?;
            match variant.as_str() {
                "Decision" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Kind", variant: "Decision" }),
                "Principle" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Kind", variant: "Principle" }),
                "Correction" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Kind", variant: "Correction" }),
                "Clarification" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Kind", variant: "Clarification" }),
                "Constraint" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Kind", variant: "Constraint" }),
                _ => Err(nota_codec::Error::UnknownVariant { enum_name: "Kind", got: variant }),
            }
        } else {
            let variant = decoder.read_pascal_identifier()?;
            match variant.as_str() {
                "Decision" => Ok(Self::Decision),
                "Principle" => Ok(Self::Principle),
                "Correction" => Ok(Self::Correction),
                "Clarification" => Ok(Self::Clarification),
                "Constraint" => Ok(Self::Constraint),
                _ => Err(nota_codec::Error::UnknownVariant { enum_name: "Kind", got: variant }),
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Magnitude {
    Minimum,
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
    Maximum,
}

impl NotaDecode for Magnitude {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        if decoder.peek_is_record_start()? {
            let variant = decoder.peek_record_head()?;
            match variant.as_str() {
                "Minimum" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Magnitude", variant: "Minimum" }),
                "VeryLow" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Magnitude", variant: "VeryLow" }),
                "Low" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Magnitude", variant: "Low" }),
                "Medium" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Magnitude", variant: "Medium" }),
                "High" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Magnitude", variant: "High" }),
                "VeryHigh" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Magnitude", variant: "VeryHigh" }),
                "Maximum" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Magnitude", variant: "Maximum" }),
                _ => Err(nota_codec::Error::UnknownVariant { enum_name: "Magnitude", got: variant }),
            }
        } else {
            let variant = decoder.read_pascal_identifier()?;
            match variant.as_str() {
                "Minimum" => Ok(Self::Minimum),
                "VeryLow" => Ok(Self::VeryLow),
                "Low" => Ok(Self::Low),
                "Medium" => Ok(Self::Medium),
                "High" => Ok(Self::High),
                "VeryHigh" => Ok(Self::VeryHigh),
                "Maximum" => Ok(Self::Maximum),
                _ => Err(nota_codec::Error::UnknownVariant { enum_name: "Magnitude", got: variant }),
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    Topics,
    Records(Entry),
}

impl NotaDecode for Observation {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        if decoder.peek_is_record_start()? {
            let variant = decoder.peek_record_head()?;
            match variant.as_str() {
                "Topics" => Err(nota_codec::Error::UnitVariantInRecordForm { enum_name: "Observation", variant: "Topics" }),
                "Records" => {
                    decoder.expect_record_head("Records")?;
                    let records = <Entry as NotaDecode>::decode(decoder)?;
                    decoder.expect_record_end()?;
                    Ok(Self::Records(records))
                }
                _ => Err(nota_codec::Error::UnknownVariant { enum_name: "Observation", got: variant }),
            }
        } else {
            let variant = decoder.read_pascal_identifier()?;
            match variant.as_str() {
                "Topics" => Ok(Self::Topics),
                "Records" => Err(nota_codec::Error::DataVariantWithoutRecord { enum_name: "Observation", variant: "Records" }),
                _ => Err(nota_codec::Error::UnknownVariant { enum_name: "Observation", got: variant }),
            }
        }
    }
}

}
