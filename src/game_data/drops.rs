//! Item drops declared by the game's BuildingEntity prefabs.

#[derive(Debug, Clone, PartialEq)]
pub struct ItemDrop {
    pub id: String,
    pub chance: f32,
    pub condition_min: f32,
    pub condition_max: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropSource {
    pub name: String,
    pub drops: Vec<ItemDrop>,
    pub always_drops: Vec<ItemDrop>,
    pub item_categories: Vec<String>,
    pub guaranteed_drop_amount: i32,
    pub drop_chance_multiplier: f32,
}

impl DropSource {
    /// Probability of selecting at least one item from a category pool.
    pub fn category_probability_for(
        &self,
        item_category: &str,
        category_pool_size: usize,
    ) -> Option<f64> {
        let category_choices = self.item_categories.len();
        let matching_categories = self
            .item_categories
            .iter()
            .filter(|category| category.as_str() == item_category)
            .count();
        if self.guaranteed_drop_amount <= 0
            || category_choices == 0
            || matching_categories == 0
            || category_pool_size == 0
        {
            return None;
        }
        let chance_per_pick =
            matching_categories as f64 / category_choices as f64 / category_pool_size as f64;
        Some(1.0 - (1.0 - chance_per_pick).powi(self.guaranteed_drop_amount))
    }

    /// Probability that this source produces at least one matching item.
    ///
    /// Category selections are uniform over the listed categories and then
    /// over items in that category. Each guaranteed pick rebuilds the pool,
    /// so picks are independent and an item can be selected more than once.
    pub fn probability_for(
        &self,
        item_id: &str,
        item_category: &str,
        category_pool_size: usize,
    ) -> Option<f64> {
        if self.always_drops.iter().any(|drop| drop.id == item_id) {
            return Some(1.0);
        }

        let mut no_drop = 1.0;
        let mut has_drop = false;
        for drop in self.drops.iter().filter(|drop| drop.id == item_id) {
            has_drop = true;
            let chance =
                (f64::from(drop.chance) * f64::from(self.drop_chance_multiplier)).clamp(0.0, 1.0);
            no_drop *= 1.0 - chance;
        }

        if let Some(pool_probability) =
            self.category_probability_for(item_category, category_pool_size)
        {
            has_drop = true;
            no_drop *= 1.0 - pool_probability;
        }

        has_drop.then_some(1.0 - no_drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drop_source(
        drops: Vec<ItemDrop>,
        always_drops: Vec<ItemDrop>,
        item_categories: Vec<&str>,
        guaranteed_drop_amount: i32,
        drop_chance_multiplier: f32,
    ) -> DropSource {
        DropSource {
            name: "test".into(),
            drops,
            always_drops,
            item_categories: item_categories.into_iter().map(str::to_owned).collect(),
            guaranteed_drop_amount,
            drop_chance_multiplier,
        }
    }

    fn item_drop(id: &str, chance: f32) -> ItemDrop {
        ItemDrop {
            id: id.into(),
            chance,
            condition_min: 0.0,
            condition_max: 1.0,
        }
    }

    #[test]
    fn category_picks_have_uniform_computed_item_chances() {
        let source = drop_source(Vec::new(), Vec::new(), vec!["tools", "medical"], 2, 1.0);
        let probability = source.probability_for("knife", "tools", 4).unwrap();
        assert!((probability - (1.0 - (7.0_f64 / 8.0).powi(2))).abs() < f64::EPSILON);
        assert_eq!(source.probability_for("knife", "food", 4), None);
    }

    #[test]
    fn explicit_and_category_chances_combine_and_guaranteed_drops_are_certain() {
        let source = drop_source(
            vec![item_drop("knife", 0.25), item_drop("knife", 0.25)],
            Vec::new(),
            vec!["tools"],
            1,
            2.0,
        );
        let probability = source.probability_for("knife", "tools", 4).unwrap();
        assert!((probability - 0.8125).abs() < f64::EPSILON);

        let guaranteed = drop_source(
            Vec::new(),
            vec![item_drop("knife", 0.0)],
            Vec::new(),
            0,
            1.0,
        );
        assert_eq!(guaranteed.probability_for("knife", "tools", 4), Some(1.0));
    }
}
