//! Exact redistribution of stored items; equipped holders never move.

use super::{group_items, item_weights};
use crate::game_data::ItemStats;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

const SEARCH_LIMIT: usize = 200_000;

/// Objective for redistributing indivisible items between equipped containers.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BalanceRule {
    /// Minimize the contribution of contents to the game's carrying load.
    #[default]
    CarryWeight,
    /// Minimize differences in the total stored weight.
    EqualWeight,
    /// Minimize differences in the used fraction of container capacity.
    EqualFill,
}

impl BalanceRule {
    /// Rules offered in the inventory.
    pub const ALL: [Self; 3] = [Self::CarryWeight, Self::EqualWeight, Self::EqualFill];

    /// Translatable interface label.
    pub fn label(self) -> &'static str {
        match self {
            Self::CarryWeight => "Minimum carrying load",
            Self::EqualWeight => "Equal stored weight",
            Self::EqualFill => "Equal fill percentage",
        }
    }
}

struct Bin<'a> {
    group: usize,
    stats: &'a ItemStats,
    capacity: f64,
    multiplier: f64,
}

struct Cargo {
    index: usize,
    weight: f64,
    destinations: Vec<usize>,
}

struct Search<'a> {
    bins: &'a [Bin<'a>],
    cargo: &'a [Cargo],
    rule: BalanceRule,
    best: Vec<usize>,
    best_score: f64,
    nodes: usize,
    seen: HashSet<(usize, usize, Vec<u64>)>,
    remaining: Vec<f64>,
    reachable: Vec<Vec<f64>>,
}

impl Search<'_> {
    fn score(&self, loads: &[f64]) -> f64 {
        match self.rule {
            BalanceRule::CarryWeight => loads
                .iter()
                .zip(self.bins)
                .map(|(load, bin)| load * bin.multiplier)
                .sum(),
            BalanceRule::EqualWeight => loads.iter().map(|load| load * load).sum(),
            BalanceRule::EqualFill => {
                let fills: Vec<_> = loads
                    .iter()
                    .zip(self.bins)
                    .map(|(load, bin)| {
                        if bin.capacity == 0.0 {
                            0.0
                        } else {
                            load / bin.capacity
                        }
                    })
                    .collect();
                let mean = fills.iter().sum::<f64>() / fills.len() as f64;
                fills.iter().map(|fill| (fill - mean).powi(2)).sum()
            }
        }
    }

    fn lower_bound(&self, depth: usize, loads: &[f64]) -> f64 {
        let upper: Vec<_> = loads
            .iter()
            .zip(self.bins)
            .zip(&self.reachable[depth])
            .map(|((&load, bin), &reachable)| bin.capacity.min(load + reachable))
            .collect();
        let total = loads.iter().sum::<f64>() + self.remaining[depth];
        if upper.iter().sum::<f64>() < total - 1e-10 {
            return f64::INFINITY;
        }
        match self.rule {
            BalanceRule::CarryWeight => {
                let restricted = self.score(loads)
                    + self.cargo[depth..]
                        .iter()
                        .map(|item| {
                            item.weight
                                * item
                                    .destinations
                                    .iter()
                                    .map(|&bin| self.bins[bin].multiplier)
                                    .fold(f64::INFINITY, f64::min)
                        })
                        .sum::<f64>();
                let mut order: Vec<_> = (0..self.bins.len()).collect();
                order.sort_by(|&a, &b| self.bins[a].multiplier.total_cmp(&self.bins[b].multiplier));
                let mut remaining = self.remaining[depth];
                let mut bound = self.score(loads);
                for bin in order {
                    let added = remaining.min((upper[bin] - loads[bin]).max(0.0));
                    bound += added * self.bins[bin].multiplier;
                    remaining -= added;
                }
                restricted.max(bound)
            }
            BalanceRule::EqualWeight => {
                let mut low = loads.iter().copied().fold(f64::INFINITY, f64::min);
                let mut high = upper.iter().copied().fold(0.0, f64::max);
                for _ in 0..40 {
                    let level = (low + high) / 2.0;
                    let used: f64 = loads
                        .iter()
                        .zip(&upper)
                        .map(|(&lower, &upper)| level.clamp(lower, upper))
                        .sum();
                    if used > total {
                        high = level;
                    } else {
                        low = level;
                    }
                }
                loads
                    .iter()
                    .zip(&upper)
                    .map(|(&lower, &upper)| low.clamp(lower, upper).powi(2))
                    .sum()
            }
            BalanceRule::EqualFill => self.fill_bound(loads, &upper, total),
        }
    }

    fn fill_bound(&self, loads: &[f64], upper: &[f64], total: f64) -> f64 {
        let capacities: Vec<_> = self.bins.iter().map(|bin| bin.capacity).collect();
        let ratio = |weight, capacity| {
            if capacity == 0.0 {
                0.0
            } else {
                weight / capacity
            }
        };
        let lower: Vec<_> = loads
            .iter()
            .zip(&capacities)
            .map(|(&w, &c)| ratio(w, c))
            .collect();
        let upper: Vec<_> = upper
            .iter()
            .zip(&capacities)
            .map(|(&w, &c)| ratio(w, c))
            .collect();
        let total_capacity = capacities.iter().sum::<f64>();
        if total_capacity == 0.0 {
            return if total == 0.0 { 0.0 } else { f64::INFINITY };
        }
        let level = total / total_capacity;
        let mut fills: Vec<_> = lower
            .iter()
            .zip(&upper)
            .map(|(&l, &u)| level.clamp(l, u))
            .collect();
        let mut remaining = total
            - fills
                .iter()
                .zip(&capacities)
                .map(|(f, c)| f * c)
                .sum::<f64>();
        for i in 0..fills.len() {
            let added = remaining.clamp(
                (lower[i] - fills[i]) * capacities[i],
                (upper[i] - fills[i]) * capacities[i],
            );
            fills[i] += ratio(added, capacities[i]);
            remaining -= added;
        }
        let count = fills.len() as f64;
        let mut mean = fills.iter().sum::<f64>() / count;
        // Pair updates improve a continuous relaxation while preserving total
        // weight. The supporting plane below is a valid bound even if these
        // bounded iterations have not reached that relaxation's optimum.
        for _ in 0..24 {
            let mut largest_change = 0.0_f64;
            for i in 0..fills.len() {
                for j in 0..i {
                    if capacities[i] == 0.0 || capacities[j] == 0.0 {
                        continue;
                    }
                    let a = 1.0 / capacities[i];
                    let b = 1.0 / capacities[j];
                    let curvature = a * a + b * b - (a - b).powi(2) / count;
                    let change = (-((fills[i] - mean) * a - (fills[j] - mean) * b) / curvature)
                        .clamp(
                            ((lower[i] - fills[i]) / a).max((fills[j] - upper[j]) / b),
                            ((upper[i] - fills[i]) / a).min((fills[j] - lower[j]) / b),
                        );
                    fills[i] = (fills[i] + change * a).clamp(lower[i], upper[i]);
                    fills[j] = (fills[j] - change * b).clamp(lower[j], upper[j]);
                    mean += change * (a - b) / count;
                    largest_change = largest_change.max(change.abs());
                }
            }
            if largest_change < 1e-10 {
                break;
            }
        }
        let mean = fills.iter().sum::<f64>() / count;
        let gradient: Vec<_> = fills.iter().map(|fill| 2.0 * (fill - mean)).collect();
        let mut order: Vec<_> = (0..fills.len()).filter(|&i| capacities[i] > 0.0).collect();
        order.sort_by(|&a, &b| {
            (gradient[a] / capacities[a]).total_cmp(&(gradient[b] / capacities[b]))
        });
        let mut remaining = total - loads.iter().sum::<f64>();
        let mut bound: f64 = fills.iter().map(|fill| (fill - mean).powi(2)).sum();
        for i in 0..fills.len() {
            bound += gradient[i] * (lower[i] - fills[i]);
        }
        // Minimize the supporting plane over all continuous feasible loads.
        for i in order {
            let added = remaining
                .min((upper[i] - lower[i]) * capacities[i])
                .max(0.0);
            bound += gradient[i] * added / capacities[i];
            remaining -= added;
        }
        bound.max(0.0)
    }

    fn visit(
        &mut self,
        depth: usize,
        loads: &mut [f64],
        assignment: &mut [usize],
    ) -> Result<(), String> {
        self.nodes += 1;
        if self.nodes > SEARCH_LIMIT || depth > 256 {
            return Err("Balancing search limit reached. No items were moved; pin more items as favourites or choose another rule.".into());
        }
        if self.lower_bound(depth, loads) >= self.best_score - 1e-10 {
            return Ok(());
        }
        let minimum_bin = depth
            .checked_sub(1)
            .filter(|&previous| {
                depth < self.cargo.len()
                    && self.cargo[previous].weight == self.cargo[depth].weight
                    && self.cargo[previous].destinations == self.cargo[depth].destinations
            })
            .map_or(0, |previous| assignment[previous]);
        if !self.seen.insert((
            depth,
            minimum_bin,
            loads.iter().map(|v| v.to_bits()).collect(),
        )) {
            return Ok(());
        }
        if depth == self.cargo.len() {
            let score = self.score(loads);
            if score < self.best_score {
                self.best_score = score;
                self.best.copy_from_slice(assignment);
            }
            return Ok(());
        }
        let weight = self.cargo[depth].weight;
        let mut destinations = self.cargo[depth].destinations.clone();
        if self.rule != BalanceRule::CarryWeight {
            destinations.sort_by(|&a, &b| {
                let projected = |bin: usize| {
                    let mut next = loads.to_vec();
                    next[bin] += weight;
                    self.score(&next)
                };
                projected(a).total_cmp(&projected(b))
            });
        }
        for bin in destinations {
            if bin < minimum_bin {
                continue;
            }
            let previous = loads[bin];
            let next = previous + weight;
            if next > self.bins[bin].capacity {
                continue;
            }
            loads[bin] = next;
            assignment[depth] = bin;
            self.visit(depth + 1, loads, assignment)?;
            loads[bin] = previous;
        }
        Ok(())
    }
}

/// Finds an optimum under container capacity, per-item weight and tag restrictions.
/// Holders and correctly stored specialized contents stay in place. Favourites
/// can also be pinned. A failed or exhausted search leaves the document untouched.
/// Returns the number of items moved.
pub fn balance_containers(
    data: &mut Value,
    stats: &HashMap<String, ItemStats>,
    rule: BalanceRule,
    pin_favourites: bool,
) -> Result<usize, String> {
    let items = data
        .get("items")
        .and_then(Value::as_array)
        .ok_or("Missing items array")?;
    let components = data
        .get("itemComponents")
        .and_then(Value::as_array)
        .ok_or("Missing itemComponents array")?;
    if components.len() != items.len() {
        return Err("Items and component arrays must be aligned before balancing".into());
    }
    let groups = group_items(data);
    let mut bins = Vec::new();
    for (group_index, group) in groups.iter().enumerate() {
        let holder = &items[group.indices[0]];
        let metadata = holder
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| stats.get(id));
        let Some(metadata) = metadata else {
            if group.indices.len() > 1 {
                return Err(format!(
                    "Missing container metadata for item #{}",
                    group.indices[0]
                ));
            }
            continue;
        };
        let Some(capacity) = metadata.container_capacity else {
            if group.indices.len() > 1 {
                return Err(format!("Item #{} is not a container", group.indices[0]));
            }
            continue;
        };
        let max_item_weight = metadata
            .container_max_item_weight
            .ok_or("Missing container item weight limit")?;
        let multiplier = metadata
            .container_encumbrance
            .ok_or("Missing container encumbrance multiplier")?;
        if [capacity, max_item_weight, multiplier]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
            || metadata.container_tag_restriction.is_none()
        {
            return Err(format!(
                "Invalid or incomplete container metadata for item #{}",
                group.indices[0]
            ));
        }
        bins.push(Bin {
            group: group_index,
            stats: metadata,
            capacity: f64::from(capacity),
            multiplier: f64::from(multiplier),
        });
    }
    if bins.len() < 2 {
        return Err("At least two equipped containers are needed".into());
    }
    let mut loads = vec![0.0; bins.len()];
    let mut cargo = Vec::new();
    let mut original = HashMap::new();
    let mut initially_valid = true;
    for (bin_index, bin) in bins.iter().enumerate() {
        let contents: Vec<_> = groups[bin.group]
            .indices
            .iter()
            .skip(1)
            .map(|&index| {
                let weights = item_weights(data, index, stats);
                let weight = weights
                    .game
                    .filter(|w| weights.game_complete && w.is_finite() && *w >= 0.0)
                    .ok_or_else(|| format!("Incomplete weight for item #{index}"))?;
                Ok((index, f64::from(weight)))
            })
            .collect::<Result<_, String>>()?;
        let preserve_contents = bin
            .stats
            .container_tag_restriction
            .as_ref()
            .is_some_and(|tags| !tags.is_empty())
            && contents.iter().map(|(_, weight)| weight).sum::<f64>() <= bin.capacity;
        for (index, weight) in contents {
            let item = &items[index];
            let metadata = item
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| stats.get(id))
                .ok_or_else(|| format!("Missing metadata for item #{index}"))?;
            if !item
                .get("condition")
                .and_then(Value::as_f64)
                .is_some_and(f64::is_finite)
            {
                return Err(format!("Missing or invalid condition for item #{index}"));
            }
            let destinations: Vec<_> = bins
                .iter()
                .enumerate()
                .filter_map(|(i, bin)| {
                    (bin.stats.accepts_item(metadata) == Some(true)
                        && bin
                            .stats
                            .container_max_item_weight
                            .is_some_and(|max| weight <= f64::from(max))
                        && weight <= bin.capacity)
                        .then_some(i)
                })
                .collect();
            let pinned = (preserve_contents && destinations.contains(&bin_index))
                || (pin_favourites
                    && item.get("favourited").and_then(Value::as_bool) == Some(true));
            if pinned && !destinations.contains(&bin_index) {
                return Err(format!(
                    "Pinned item #{index} violates its current container's restrictions"
                ));
            }
            initially_valid &= destinations.contains(&bin_index);
            loads[bin_index] += weight;
            original.insert(index, bin_index);
            if !pinned {
                if destinations.is_empty() {
                    return Err(format!("No container accepts item #{index}"));
                }
                cargo.push(Cargo {
                    index,
                    weight,
                    destinations,
                });
            }
        }
        initially_valid &= loads[bin_index] <= bin.capacity;
    }
    cargo.sort_by(|a, b| {
        a.destinations
            .len()
            .cmp(&b.destinations.len())
            .then_with(|| b.weight.total_cmp(&a.weight))
            .then_with(|| a.destinations.cmp(&b.destinations))
    });
    if rule == BalanceRule::CarryWeight {
        for item in &mut cargo {
            item.destinations
                .sort_by(|&a, &b| bins[a].multiplier.total_cmp(&bins[b].multiplier));
        }
    }
    let initial: Vec<_> = cargo.iter().map(|item| original[&item.index]).collect();
    let mut remaining = vec![0.0; cargo.len() + 1];
    let mut reachable = vec![vec![0.0; bins.len()]; cargo.len() + 1];
    for (depth, item) in cargo.iter().enumerate().rev() {
        remaining[depth] = remaining[depth + 1] + item.weight;
        reachable[depth] = reachable[depth + 1].clone();
        for &bin in &item.destinations {
            reachable[depth][bin] += item.weight;
        }
    }
    let mut search = Search {
        bins: &bins,
        cargo: &cargo,
        rule,
        best: initial.clone(),
        best_score: f64::INFINITY,
        nodes: 0,
        seen: HashSet::new(),
        remaining,
        reachable,
    };
    if initially_valid {
        search.best_score = search.score(&loads);
    }
    for item in &cargo {
        loads[original[&item.index]] -= item.weight;
    }
    if loads
        .iter()
        .zip(&bins)
        .any(|(load, bin)| *load > bin.capacity)
    {
        return Err("Pinned items exceed their container's capacity".into());
    }
    search.visit(0, &mut loads, &mut initial.clone())?;
    if !search.best_score.is_finite() {
        return Err("No valid distribution fits the equipped containers".into());
    }
    let moved = cargo
        .iter()
        .zip(&search.best)
        .filter(|(item, bin)| original[&item.index] != **bin)
        .count();
    if moved == 0 {
        return Ok(0);
    }
    let mut destinations: HashMap<_, _> = original
        .iter()
        .map(|(&index, &bin)| (index, bins[bin].group))
        .collect();
    for (item, &bin) in cargo.iter().zip(&search.best) {
        destinations.insert(item.index, bins[bin].group);
    }
    let mut new_items = Vec::with_capacity(items.len());
    let mut new_components = Vec::with_capacity(items.len());
    for (group_index, group) in groups.iter().enumerate() {
        let holder = group.indices[0];
        new_items.push(items[holder].clone());
        new_components.push(components[holder].clone());
        for (index, item) in items.iter().enumerate() {
            if destinations.get(&index) == Some(&group_index)
                || (!destinations.contains_key(&index) && group.indices[1..].contains(&index))
            {
                let mut item = item.clone();
                group.location.apply(&mut item);
                new_items.push(item);
                new_components.push(components[index].clone());
            }
        }
    }
    data["items"] = Value::Array(new_items);
    data["itemComponents"] = Value::Array(new_components);
    Ok(moved)
}
