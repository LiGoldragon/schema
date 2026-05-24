use crate::{Name, Upgrade};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Feature {
    Reply(Vec<Name>),
    Event(EventFeature),
    Observable(ObservableFeature),
    Upgrade(Upgrade),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventFeature {
    stream: Option<Name>,
    events: Vec<Name>,
}

impl EventFeature {
    pub fn new(stream: Option<Name>, events: Vec<Name>) -> Self {
        Self { stream, events }
    }

    pub fn stream(&self) -> Option<&Name> {
        self.stream.as_ref()
    }

    pub fn events(&self) -> &[Name] {
        &self.events
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservableFeature {
    filter: Option<String>,
    operation_event: Option<Name>,
    effect_event: Option<Name>,
}

impl ObservableFeature {
    pub fn new(
        filter: Option<String>,
        operation_event: Option<Name>,
        effect_event: Option<Name>,
    ) -> Self {
        Self {
            filter,
            operation_event,
            effect_event,
        }
    }

    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    pub fn operation_event(&self) -> Option<&Name> {
        self.operation_event.as_ref()
    }

    pub fn effect_event(&self) -> Option<&Name> {
        self.effect_event.as_ref()
    }
}
