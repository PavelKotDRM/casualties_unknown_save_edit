//! Collected effects, timed operations and normalization.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum RawChange {
    Add(Lin),
    Mul(Lin),
    Set(Lin),
    Trigger,
}

impl RawChange {
    pub(super) fn map(self, f: impl Fn(Lin) -> Lin) -> Self {
        match self {
            Self::Add(n) => Self::Add(f(n)),
            Self::Mul(n) => Self::Mul(f(n)),
            Self::Set(n) => Self::Set(f(n)),
            Self::Trigger => Self::Trigger,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Raw {
    pub(super) target: Target,
    pub(super) stat: String,
    pub(super) change: RawChange,
    pub(super) seconds: Option<Lin>,
    pub(super) whole_item: bool,
    pub(super) conditional: bool,
}

/// What a method does, in terms of its own float parameter.
#[derive(Debug, Default)]
pub(super) struct Out {
    pub(super) raw: Vec<Raw>,
    /// Change of the used item's condition.
    pub(super) condition: Lin,
    pub(super) contents: Vec<(ContentsUse, Lin)>,
    /// The instruction being evaluated is inside a conditional branch.
    pub(super) conditional: bool,
}

impl Out {
    pub(super) fn add(&mut self, target: Target, stat: &str, change: RawChange) {
        self.raw.push(Raw {
            target,
            stat: stat.to_owned(),
            change,
            seconds: None,
            whole_item: false,
            conditional: self.conditional,
        });
    }

    pub(super) fn store(&mut self, owner: &str, name: &str, token: u32, object: &Val, value: Val) {
        let change = match value {
            Val::Rel(field, change) if field == token => change,
            Val::Num(n) => RawChange::Set(n),
            _ => return,
        };
        let target = match owner {
            "Body" => Target::Body,
            "Limb" => match object {
                Val::Obj(Obj::Limb(i)) => Target::Limb(*i),
                Val::Obj(Obj::EachLimb) => Target::EachLimb,
                _ => Target::TreatedLimb,
            },
            "Item" => {
                if let ("condition", RawChange::Add(n)) = (name, change) {
                    self.condition = self.condition.plus(n);
                }
                return;
            }
            // Components hold internal state; only their additive changes are meaningful.
            "CustomItemBehaviour" => return,
            _ if matches!(change, RawChange::Add(_)) => Target::Component(owner.to_owned()),
            _ => return,
        };
        self.add(target, name, change);
    }

    pub(super) fn merge(&mut self, other: Out) {
        let conditional = self.conditional;
        self.raw.extend(other.raw.into_iter().map(|mut raw| {
            raw.conditional |= conditional;
            raw
        }));
        self.condition = self.condition.plus(other.condition);
        self.contents.extend(other.contents);
    }

    /// Merges a timed operation that runs `inner` once per second for `duration` seconds.
    pub(super) fn merge_timed(&mut self, inner: Out, duration: Lin) {
        for mut raw in inner.raw {
            raw.conditional |= self.conditional;
            // A conditional change keeps its per-occurrence amount.
            if let (RawChange::Add(n), false) = (raw.change, raw.conditional) {
                match n.times(duration) {
                    Some(total) => raw.change = RawChange::Add(total),
                    None => continue,
                }
            }
            raw.seconds = Some(duration);
            self.raw.push(raw);
        }
    }

    /// Removes the dependency on a lambda's own float parameter: per whole
    /// item when the item's condition depends on it, otherwise at `x = 1`
    /// (a fully completed minigame).
    pub(super) fn normalize(self, has_float: bool) -> Self {
        if !has_float {
            return self;
        }
        if self.condition.a == 0.0 {
            return self.at(1.0);
        }
        let per_item = 1.0 / self.condition.a.abs();
        let fix = |n: Lin| Lin::constant(n.a * per_item);
        Self {
            raw: self
                .raw
                .into_iter()
                .map(|mut r| {
                    r.change = r.change.map(fix);
                    r.seconds = r.seconds.map(fix);
                    r.whole_item = true;
                    r
                })
                .collect(),
            condition: Lin::ZERO,
            contents: self
                .contents
                .into_iter()
                .map(|(k, n)| (k, fix(n)))
                .collect(),
            conditional: false,
        }
    }

    /// Evaluates every amount at `x`.
    pub(super) fn at(self, x: f32) -> Self {
        let fix = |n: Lin| Lin::constant(n.at(x));
        Self {
            raw: self
                .raw
                .into_iter()
                .map(|mut r| {
                    r.change = r.change.map(fix);
                    r.seconds = r.seconds.map(fix);
                    r
                })
                .collect(),
            condition: fix(self.condition),
            contents: self
                .contents
                .into_iter()
                .map(|(k, n)| (k, fix(n)))
                .collect(),
            conditional: false,
        }
    }

    pub(super) fn effects(&self) -> Vec<Effect> {
        let list = self
            .raw
            .iter()
            .map(|r| Effect {
                target: r.target.clone(),
                stat: r.stat.clone(),
                change: match r.change {
                    RawChange::Add(n) => Change::Add(n.b),
                    RawChange::Mul(n) => Change::Multiply(n.b),
                    RawChange::Set(n) => Change::Set(n.b),
                    RawChange::Trigger => Change::Trigger,
                },
                seconds: r.seconds.map(|s| s.b).filter(|s| *s > 0.0),
                whole_item: r.whole_item,
                conditional: r.conditional,
            })
            .collect();
        merge(list)
    }
}

/// Combines additions to the same stat and drops changes that do nothing.
pub(super) fn merge(effects: Vec<Effect>) -> Vec<Effect> {
    let mut out: Vec<Effect> = Vec::new();
    let mut triggers = HashSet::new();
    for effect in effects {
        if let Change::Add(v) = effect.change
            && let Some(existing) = out.iter_mut().find(|e| {
                e.target == effect.target
                    && e.stat == effect.stat
                    && e.seconds == effect.seconds
                    && e.whole_item == effect.whole_item
                    && e.conditional == effect.conditional
                    && matches!(e.change, Change::Add(_))
            })
        {
            if let Change::Add(total) = &mut existing.change {
                *total += v;
            }
            continue;
        }
        if effect.change == Change::Trigger
            && !triggers.insert((effect.target.clone(), effect.stat.clone()))
        {
            continue;
        }
        out.push(effect);
    }
    out.retain(|e| match e.change {
        Change::Add(v) => v.is_finite() && v.abs() > 1e-4,
        Change::Multiply(v) => v.is_finite() && (v - 1.0).abs() > 1e-4,
        Change::Set(v) => v.is_finite(),
        Change::Trigger => true,
    });
    out
}
