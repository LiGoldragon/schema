use crate::{Name, Upgrade};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Feature {
    Reply(Vec<Name>),
    Event(EventFeature),
    Observable(ObservableFeature),
    Upgrade(Upgrade),

    /// Authored effect-table feature per /343 §3 + /345 §8 correction.
    ///
    /// `EffectTable` declares the closed mapping from operation root
    /// (or actor ACTION variant in the case of internal-channel
    /// schemas per /346 §10) to the EFFECT TYPE that the actor's
    /// `handle` method produces. The composer emits an `Effect` enum
    /// and a `effect_for_<action>` dispatch table from this feature.
    ///
    /// /345 §8 correction: this feature lives in the ACTOR's own
    /// schema (`spirit-recorder.schema`), NOT in the wire schema. The
    /// wire schema doesn't know about effects; effects are
    /// daemon-internal.
    EffectTable(EffectTableFeature),

    /// Per-effect declared fan-out outputs per /343 §4 + /346 §10.
    ///
    /// Each row says: when this effect fires, fan out to these
    /// downstream targets. Each target is `(MethodTag, ActorType,
    /// ActorMethod)` for a storage/subscription/other-actor call, or
    /// `(Reply, ReplyVariant)` for a wire reply, or
    /// `(Subscribe, SubscriberSet, Method)` for a subscription
    /// publication, etc.
    ///
    /// The closed set of output kinds is intentional: every fan-out
    /// output is structurally typed and survives schema-diff inspection.
    FanOutTargets(FanOutTargetsFeature),

    /// Storage descriptor feature per /343 §8 item 4 + /346 §4.
    ///
    /// Names the closed set of storage tables this schema owns, along
    /// with the key + value types per table. The composer emits
    /// `TableDescriptor`s + a `StorageDescriptor` struct binding the
    /// daemon to the redb table layout. The version marker per
    /// /346 §4 is part of every StorageDescriptor.
    StorageDescriptor(StorageDescriptorFeature),
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

/// Closed action -> effect mapping per /343 §3 + /345 §8 + /346 §10.
///
/// Each entry is `(action_root_or_variant, effect_type)`. The composer
/// emits an `Effect` enum (variants from `entries[i].1`) plus the
/// `effect_for_<action>` dispatcher that takes the action variant and
/// returns the corresponding effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectTableFeature {
    entries: Vec<EffectTableEntry>,
}

impl EffectTableFeature {
    pub fn new(entries: Vec<EffectTableEntry>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[EffectTableEntry] {
        &self.entries
    }
}

/// `(action_root_or_variant, effect_type)` per /343 §3.
///
/// The first name is the ACTION variant or operation root (depending
/// on whether this is an internal actor schema or a wire schema; per
/// /345 §8 the canonical form is internal). The second name is the
/// effect type that the actor's handler produces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectTableEntry {
    action: Name,
    effect: Name,
}

impl EffectTableEntry {
    pub fn new(action: Name, effect: Name) -> Self {
        Self { action, effect }
    }

    pub fn action(&self) -> &Name {
        &self.action
    }

    pub fn effect(&self) -> &Name {
        &self.effect
    }
}

/// Per-effect fan-out outputs per /343 §4 + /346 §10.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FanOutTargetsFeature {
    entries: Vec<FanOutTargetsEntry>,
}

impl FanOutTargetsFeature {
    pub fn new(entries: Vec<FanOutTargetsEntry>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[FanOutTargetsEntry] {
        &self.entries
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FanOutTargetsEntry {
    effect: Name,
    outputs: Vec<FanOutOutputDeclaration>,
}

impl FanOutTargetsEntry {
    pub fn new(effect: Name, outputs: Vec<FanOutOutputDeclaration>) -> Self {
        Self { effect, outputs }
    }

    pub fn effect(&self) -> &Name {
        &self.effect
    }

    pub fn outputs(&self) -> &[FanOutOutputDeclaration] {
        &self.outputs
    }
}

/// One fan-out output line per /343 §4 + /346 §10.
///
/// Three forms exist:
///
/// - `Reply` --- materialises as a wire reply variant. Single name (the
///   reply variant); enforced to match a `Feature::Reply` declaration
///   per /343 §8 item 3.
/// - `Actor` --- a fan-out to another actor's method. Carries
///   `(method_tag, actor_type, actor_method)`.
/// - `Subscribers` --- a fan-out to a set of subscribers (e.g.
///   `ObserverSet`). Carries `(actor_type, dispatch_method)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FanOutOutputDeclaration {
    Reply {
        variant: Name,
    },
    Actor {
        method_tag: Name,
        actor_type: Name,
        actor_method: Name,
    },
    Subscribers {
        actor_type: Name,
        dispatch_method: Name,
    },
}

/// Closed set of storage tables this schema owns per /343 §8 item 4.
///
/// Each entry names a logical table and the table-layout type (declared
/// elsewhere in the namespace as a record `[Key Value]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageDescriptorFeature {
    entries: Vec<StorageDescriptorEntry>,
}

impl StorageDescriptorFeature {
    pub fn new(entries: Vec<StorageDescriptorEntry>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[StorageDescriptorEntry] {
        &self.entries
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageDescriptorEntry {
    logical_name: Name,
    table_type: Name,
}

impl StorageDescriptorEntry {
    pub fn new(logical_name: Name, table_type: Name) -> Self {
        Self {
            logical_name,
            table_type,
        }
    }

    pub fn logical_name(&self) -> &Name {
        &self.logical_name
    }

    pub fn table_type(&self) -> &Name {
        &self.table_type
    }
}
