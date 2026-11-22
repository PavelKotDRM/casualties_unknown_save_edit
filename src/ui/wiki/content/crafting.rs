//! Game-derived crafting recipes and rules.

use super::super::effects;
use super::super::{CraftingTree, TreeNode, TreeNodeKind};
use super::*;
use crate::game_data::crafting::{CraftingIngredient, CraftingRecipe, RecipeCategory};
use std::collections::HashSet;

pub(super) fn recipes(sources: &Sources<'_>) -> Vec<Entry> {
    let mut entries = vec![rules(sources.names)];
    entries.extend(
        sources
            .crafting
            .iter()
            .map(|recipe| recipe_entry(sources.names, sources.effects, sources.crafting, recipe)),
    );
    entries
}

fn rules(names: Names<'_>) -> Entry {
    let mut entry = Entry::new(format!("🛠 {}", names.text("Crafting rules")));
    entry.lists = [
        (
            "Recipe visibility",
            "Unlearned recipes appear when INT is no more than 3 below the requirement; specially known recipes always appear.",
        ),
        (
            "Ingredients",
            "Ingredients can come from inventory or nearby ground items within 10 units. Favourited items and non-empty containers are excluded.",
        ),
        (
            "Low INT",
            "At 1 INT below the requirement, half of crafts produce a result at 20–90% of its listed condition or volume. At 2 or more below, half produce nothing; at exactly 3 below, that failed attempt also injures the player.",
        ),
        (
            "Ingredient use",
            "A craft uses its ingredients before the result is rolled. Specific items lose the listed condition amount; quality ingredients lose condition in proportion to the quality supplied. Ingredients marked consumed are destroyed; liquid ingredients are drained.",
        ),
        (
            "Container results",
            "A crafted liquid container is emptied and set to 0 condition unless its recipe preserves the result liquid.",
        ),
        (
            "First craft",
            "Making a recipe for the first time raises happiness and grants INT experience.",
        ),
    ]
    .into_iter()
    .map(|(heading, text)| {
        (
            names.text(heading).into_owned(),
            vec![(names.text(text).into_owned(), Tone::Neutral)],
        )
    })
    .collect();
    entry
}

fn recipe_entry(
    names: Names<'_>,
    game_effects: &GameEffects,
    recipes: &[CraftingRecipe],
    recipe: &CraftingRecipe,
) -> Entry {
    let mut entry = Entry::new(format!(
        "🛠 {}{}",
        if recipe.is_repair {
            format!("{} ", names.text("Repair"))
        } else {
            String::new()
        },
        result_name(names, game_effects, recipe)
    ));
    if !recipe.result_is_liquid {
        entry.icon_id = Some(recipe.result_id.clone());
    }
    let mut aliases = vec![recipe.result_id.as_str()];
    for ingredient in &recipe.ingredients {
        if let Some(id) = ingredient
            .specific_id
            .as_deref()
            .or(ingredient.quality_id.as_deref())
        {
            aliases.push(id);
        }
    }
    entry.search_alias = Some(aliases.join(" "));
    entry.subtitle = format!(
        "{} · {} {} · #{}",
        names.text(category_label(recipe.category)),
        names.text("INT"),
        recipe.intelligence,
        recipe.index + 1
    );
    entry.text = if recipe.result_is_liquid {
        names
            .locale
            .and_then(|locale| locale.other(&format!("{}dsc", recipe.result_id)))
            .map(strip_rich_text)
    } else {
        names.item_description(&recipe.result_id)
    };
    entry.facts.push((
        names.text("Category").into_owned(),
        names.text(category_label(recipe.category)).into_owned(),
    ));
    entry.facts.push((
        names.text("INT required").into_owned(),
        recipe.intelligence.to_string(),
    ));
    entry.facts.push((
        names.text("Produces").into_owned(),
        result_output(names, game_effects, recipe),
    ));
    if recipe.special_known {
        entry.facts.push((
            names.text("Specially known").into_owned(),
            names.text("yes").into_owned(),
        ));
    }
    if recipe.dont_drain_result_liquid {
        entry.facts.push((
            names.text("Keeps result liquid").into_owned(),
            names.text("yes").into_owned(),
        ));
    }

    entry.lists.push((
        names.text("Ingredients").into_owned(),
        recipe
            .ingredients
            .iter()
            .map(|ingredient| {
                (
                    ingredient_line(names, game_effects, ingredient),
                    Tone::Neutral,
                )
            })
            .collect(),
    ));
    entry.crafting_tree = Some(crafting_tree(names, game_effects, recipes, recipe));
    entry
}

fn crafting_tree(
    names: Names<'_>,
    game_effects: &GameEffects,
    recipes: &[CraftingRecipe],
    recipe: &CraftingRecipe,
) -> CraftingTree {
    let mut ancestors = HashSet::from([recipe.result_id.clone()]);
    let result_name = result_name(names, game_effects, recipe);
    let mut root = TreeNode::new(
        if recipe.result_is_liquid {
            TreeNodeKind::Liquid
        } else {
            TreeNodeKind::Item
        },
        result_name,
        Some(result_detail(names, recipe)),
        (!recipe.result_is_liquid).then(|| recipe.result_id.clone()),
    );
    root.children.push(recipe_node(
        names,
        game_effects,
        recipes,
        recipe,
        0,
        &mut ancestors,
    ));
    CraftingTree {
        title: names.text("Crafting tree").into_owned(),
        root,
    }
}

fn recipe_node(
    names: Names<'_>,
    game_effects: &GameEffects,
    recipes: &[CraftingRecipe],
    recipe: &CraftingRecipe,
    depth: usize,
    ancestors: &mut HashSet<String>,
) -> TreeNode {
    let action = if recipe.is_repair {
        names.text("Repair")
    } else {
        names.text("Recipe")
    };
    let mut node = TreeNode::new(
        TreeNodeKind::Recipe,
        format!("{action} #{}", recipe.index + 1),
        Some(format!(
            "{} · {} {}",
            names.text(category_label(recipe.category)),
            names.text("INT"),
            recipe.intelligence
        )),
        None,
    );
    for ingredient in &recipe.ingredients {
        let mut input = ingredient_node(names, game_effects, ingredient);
        if let Some(id) = ingredient.specific_id.as_deref() {
            let producers: Vec<_> = recipes
                .iter()
                .filter(|candidate| {
                    candidate.result_id == id && candidate.result_is_liquid == ingredient.is_liquid
                })
                .collect();
            if ancestors.contains(id) {
                node.children.push(input);
                continue;
            }
            if depth >= 2 {
                if !producers.is_empty() {
                    input.children.push(TreeNode::new(
                        TreeNodeKind::Recipe,
                        names.text("more"),
                        Some(names.text("Deeper recipes omitted").into_owned()),
                        None,
                    ));
                }
            } else if !producers.is_empty() {
                let additional_producers = producers.len().saturating_sub(2);
                ancestors.insert(id.to_owned());
                input.children = producers
                    .iter()
                    .take(2)
                    .map(|producer| {
                        recipe_node(names, game_effects, recipes, producer, depth + 1, ancestors)
                    })
                    .collect();
                ancestors.remove(id);
                if additional_producers > 0 {
                    input.children.push(TreeNode::new(
                        TreeNodeKind::Recipe,
                        format!("+{additional_producers} {}", names.text("more")),
                        None,
                        None,
                    ));
                }
            }
        }
        node.children.push(input);
    }
    node
}

fn ingredient_node(
    names: Names<'_>,
    game_effects: &GameEffects,
    ingredient: &CraftingIngredient,
) -> TreeNode {
    if let Some(id) = &ingredient.specific_id {
        let title = if ingredient.is_liquid {
            effects::liquid_name(names, game_effects, id)
        } else {
            names.item(id)
        };
        TreeNode::new(
            if ingredient.is_liquid {
                TreeNodeKind::Liquid
            } else {
                TreeNodeKind::Item
            },
            title,
            Some(specific_ingredient_detail(names, ingredient)),
            (!ingredient.is_liquid).then(|| id.clone()),
        )
    } else {
        let quality_id = ingredient.quality_id.as_deref().unwrap_or_default();
        let key = format!("cq{quality_id}");
        let quality = names
            .locale
            .and_then(|locale| locale.other(&key))
            .map(strip_rich_text)
            .unwrap_or_else(|| prettify_id(quality_id));
        let mut node = TreeNode::new(
            TreeNodeKind::Quality,
            format!(
                "{} {quality}",
                names.text(if ingredient.is_liquid {
                    "Any liquid with"
                } else {
                    "Any item with"
                })
            ),
            Some(quality_ingredient_detail(names, ingredient)),
            None,
        );
        node.quality_filter = Some(QualityFilter {
            id: quality_id.to_owned(),
            label: quality,
            amount: ingredient.quality_amount,
            is_liquid: ingredient.is_liquid,
            ignored_id: ingredient.ignored_id.clone(),
            click_hint: names.text("Show matching items or liquids").into_owned(),
        });
        node
    }
}

fn specific_ingredient_detail(names: Names<'_>, ingredient: &CraftingIngredient) -> String {
    if ingredient.is_liquid {
        return format!("{} ml", fmt_num(f64::from(ingredient.minimum_condition)));
    }
    let mut details = vec![format!(
        "≥{}% {}",
        fmt_num(f64::from(ingredient.minimum_condition * 100.0)),
        names.text("condition")
    )];
    if ingredient.no_max_condition {
        details.push(format!("<99.1% {}", names.text("condition")));
    }
    details.push(
        names
            .text(if ingredient.destroy_item {
                "consumed"
            } else {
                "kept"
            })
            .into_owned(),
    );
    if let Some(ignored) = &ingredient.ignored_id {
        details.push(format!("{} {}", names.text("not"), names.item(ignored)));
    }
    details.join(" · ")
}

fn quality_ingredient_detail(names: Names<'_>, ingredient: &CraftingIngredient) -> String {
    let mut details = vec![format!(
        "{} {}",
        names.text("quality amount"),
        fmt_num(f64::from(ingredient.quality_amount))
    )];
    if !ingredient.is_liquid {
        details.push(format!(
            "≥{}% {}",
            fmt_num(f64::from(ingredient.minimum_condition * 100.0)),
            names.text("condition")
        ));
        if ingredient.no_max_condition {
            details.push(format!("<99.1% {}", names.text("condition")));
        }
        details.push(
            names
                .text(if ingredient.destroy_item {
                    "consumed"
                } else {
                    "kept"
                })
                .into_owned(),
        );
    }
    details.join(" · ")
}

fn result_detail(names: Names<'_>, recipe: &CraftingRecipe) -> String {
    let quantity = if recipe.result_is_liquid {
        format!("{} ml", fmt_num(f64::from(recipe.result_condition)))
    } else {
        format!(
            "{}% {}",
            fmt_num(f64::from(recipe.result_condition * 100.0)),
            names.text("condition")
        )
    };
    format!(
        "{}{}",
        if recipe.result_amount == 1 {
            String::new()
        } else {
            format!("×{} · ", recipe.result_amount)
        },
        quantity
    )
}

fn category_label(category: RecipeCategory) -> &'static str {
    match category {
        RecipeCategory::Materials => "Materials",
        RecipeCategory::Tools => "Tools",
        RecipeCategory::Medicine => "Medicine",
        RecipeCategory::Utilities => "Utilities",
        RecipeCategory::Food => "Food",
    }
}

fn result_name(names: Names<'_>, game_effects: &GameEffects, recipe: &CraftingRecipe) -> String {
    if recipe.result_is_liquid {
        effects::liquid_name(names, game_effects, &recipe.result_id)
    } else {
        names.item(&recipe.result_id)
    }
}

fn result_output(names: Names<'_>, game_effects: &GameEffects, recipe: &CraftingRecipe) -> String {
    let name = result_name(names, game_effects, recipe);
    let quantity = if recipe.result_is_liquid {
        format!("{} ml", fmt_num(f64::from(recipe.result_condition)))
    } else {
        format!(
            "{}% {}",
            fmt_num(f64::from(recipe.result_condition * 100.0)),
            names.text("condition")
        )
    };
    format!(
        "{name}{} · {quantity}",
        if recipe.result_amount == 1 {
            String::new()
        } else {
            format!(" ×{}", recipe.result_amount)
        }
    )
}

fn ingredient_line(
    names: Names<'_>,
    game_effects: &GameEffects,
    ingredient: &CraftingIngredient,
) -> String {
    let mut line = if let Some(id) = &ingredient.specific_id {
        let label = if ingredient.is_liquid {
            effects::liquid_name(names, game_effects, id)
        } else {
            names.item(id)
        };
        if ingredient.is_liquid {
            format!(
                "{label} · {} ml",
                fmt_num(f64::from(ingredient.minimum_condition))
            )
        } else {
            format!(
                "{label} · ≥{}% {}",
                fmt_num(f64::from(ingredient.minimum_condition * 100.0)),
                names.text("condition")
            )
        }
    } else {
        let quality_id = ingredient.quality_id.as_deref().unwrap_or_default();
        let key = format!("cq{quality_id}");
        let quality = names
            .locale
            .and_then(|locale| locale.other(&key))
            .map(strip_rich_text)
            .unwrap_or_else(|| prettify_id(quality_id));
        let mut line = format!(
            "{} {} · {} · {}",
            names.text(if ingredient.is_liquid {
                "Any liquid with"
            } else {
                "Any item with"
            }),
            quality,
            names.text("quality amount"),
            fmt_num(f64::from(ingredient.quality_amount))
        );
        if !ingredient.is_liquid {
            line.push_str(&format!(
                " · ≥{}% {}",
                fmt_num(f64::from(ingredient.minimum_condition * 100.0)),
                names.text("condition")
            ));
        }
        line
    };
    if ingredient.no_max_condition && !ingredient.is_liquid {
        line.push_str(&format!(" · <99.1% {}", names.text("condition")));
    }
    if !ingredient.is_liquid {
        line.push_str(&format!(
            " · {}",
            names.text(if ingredient.destroy_item {
                "consumed"
            } else {
                "kept"
            })
        ));
    }
    if !ingredient.is_liquid
        && let Some(ignored) = &ingredient.ignored_id
    {
        line.push_str(&format!(" · {} {}", names.text("not"), names.item(ignored)));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_data::effects::ItemProfile;
    use crate::ui::InterfaceLanguage;

    fn names() -> Names<'static> {
        Names {
            locale: None,
            interface_language: InterfaceLanguage::from_code(None, None),
        }
    }

    fn recipe(
        index: usize,
        result_id: &str,
        ingredients: Vec<CraftingIngredient>,
    ) -> CraftingRecipe {
        CraftingRecipe {
            index,
            intelligence: 0,
            category: RecipeCategory::Materials,
            special_known: false,
            is_repair: false,
            result_id: result_id.into(),
            result_is_liquid: false,
            result_amount: 1,
            result_condition: 1.0,
            dont_drain_result_liquid: false,
            ingredients,
        }
    }

    fn specific(id: &str) -> CraftingIngredient {
        CraftingIngredient {
            is_liquid: false,
            specific_id: Some(id.into()),
            quality_id: None,
            quality_amount: 0.0,
            minimum_condition: 0.0,
            destroy_item: true,
            ignored_id: None,
            no_max_condition: false,
        }
    }

    #[test]
    fn crafting_tree_expands_producers_but_not_quality_candidates_or_cycles() {
        let producer = recipe(0, "fiber", vec![specific("leaf")]);
        let root = recipe(1, "rope", vec![specific("fiber")]);
        let mut game_effects = GameEffects::default();
        game_effects.items.insert(
            "leaf".into(),
            ItemProfile {
                qualities: vec![("foliage".into(), 1.0)],
                ..Default::default()
            },
        );
        let quality_recipe = recipe(
            2,
            "cloth",
            vec![CraftingIngredient {
                specific_id: None,
                quality_id: Some("foliage".into()),
                quality_amount: 1.0,
                ..specific("")
            }],
        );

        let tree = crafting_tree(
            names(),
            &game_effects,
            &[producer.clone(), root.clone()],
            &root,
        );
        let fiber_node = &tree.root.children[0].children[0];
        assert_eq!(fiber_node.title, "Fiber");
        assert_eq!(fiber_node.children[0].title, "Recipe #1");
        assert_eq!(
            fiber_node.children[0].children[0].icon_id.as_deref(),
            Some("leaf")
        );

        let quality_tree = crafting_tree(
            names(),
            &game_effects,
            std::slice::from_ref(&quality_recipe),
            &quality_recipe,
        );
        let quality_node = &quality_tree.root.children[0].children[0];
        assert_eq!(quality_node.kind, TreeNodeKind::Quality);
        assert!(quality_node.children.is_empty());
        assert!(quality_node.quality_filter.is_some());

        let cycle = recipe(3, "rope", vec![specific("rope")]);
        let cycle_tree =
            crafting_tree(names(), &game_effects, std::slice::from_ref(&cycle), &cycle);
        assert!(cycle_tree.root.children[0].children[0].children.is_empty());
    }
}
