use serde::{Deserialize, de::DeserializeOwned, Serialize};
use std::fmt;

#[derive(Debug, Clone)]
pub enum ConfigError {
    UnknownArchetypeName(String)
}
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ConfigError::*;
        match self {
            UnknownArchetypeName(name) => write!(f, "no archetype by the name of {:?}", name)
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub special_users: Vec<String>,
    pub hackstead_advancements: AdvancementSet<HacksteadAdvancementSum>,
    pub plant_archetypes: Vec<PlantArchetype>,
    pub possession_archetypes: Vec<Archetype>,
}
impl Config {
    fn find_plant<S: AsRef<str>>(&self, name: &S) -> Result<&PlantArchetype, ConfigError> {
        self.plant_archetypes
            .iter()
            .find(|x| name.as_ref() == x.name)
            .ok_or(ConfigError::UnknownArchetypeName(name.as_ref().to_string()))
    }
    fn find_possession<S: AsRef<str>>(&self, name: &S) -> Result<&Archetype, ConfigError> {
        self.possession_archetypes
            .iter()
            .find(|x| name.as_ref() == x.name)
            .ok_or(ConfigError::UnknownArchetypeName(name.as_ref().to_string()))
    }
    fn find_possession_handle<S: AsRef<str>>(&self, name: &S) -> Result<ArchetypeHandle, ConfigError> {
        self.possession_archetypes
            .iter()
            .position(|x| name.as_ref() == x.name)
            .ok_or(ConfigError::UnknownArchetypeName(name.as_ref().to_string()))
    }
}

pub type ArchetypeHandle = usize;

lazy_static::lazy_static! {
    pub static ref CONFIG: Config = {
        pub fn f<T: DeserializeOwned>(p: &'static str) -> T {
            serde_json::from_str(
                &std::fs::read_to_string(format!(
                    concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/config/{}.json",
                    ),
                    p
                ))
                .unwrap_or_else(|e| panic!("opening {}: {}", p, e))
            )
            .unwrap_or_else(|e| panic!("parsing {}: {}", p, e))
        }

        Config {
            special_users: f("special_users"),
            hackstead_advancements: f("hackstead_advancements"),
            plant_archetypes: f("plant_archetypes"),
            possession_archetypes: f("possession_archetypes"),
        }
    };
}

#[derive(Deserialize, Debug, Clone)]
pub enum HacksteadAdvancementKind {
    Land { pieces: usize },
}
#[derive(Deserialize, Debug, Clone)]
pub struct HacksteadAdvancementSum {
    land: usize,
}
impl AdvancementSum for HacksteadAdvancementSum {
    type Kind = HacksteadAdvancementKind;

    fn new(unlocked: &[Advancement<Self>]) -> Self {
        Self { 
            land: unlocked.iter().map(|k| match k.kind {
                HacksteadAdvancementKind::Land { pieces } => pieces
            }).sum()
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct GotchiArchetype {
    pub base_happiness: u64,
}
#[derive(Deserialize, Debug, Clone)]
pub struct SeedArchetype {
    pub grows_into: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct KeepsakeArchetype;

#[derive(Deserialize, Debug, Clone)]
pub enum ArchetypeKind {
    Gotchi(GotchiArchetype),
    Seed(SeedArchetype),
    Keepsake(KeepsakeArchetype),
}
#[derive(Deserialize, Debug, Clone)]
pub struct Archetype {
    pub name: String,
    pub kind: ArchetypeKind,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PlantArchetype {
    pub name: String,
    pub advancements: AdvancementSet<PlantAdvancementSum>,
}
/// Recipe is generic over the way Archetypes are referred to
/// to make it easy to use Strings in the configs and ArchetypeHandles
/// at runtime
#[derive(Deserialize, Debug, Clone)]
pub struct Recipe<Handle> {
    needs: Vec<(usize, Handle)>,
    makes: Handle,
}
#[derive(Deserialize, Debug, Clone)]
pub enum PlantAdvancementKind {
    Xp { multiplier: f32 },
    YieldSpeed { multiplier: f32 },
    YieldNeighboringSize { multiplier: f32 },
    Yield { resources: Vec<(f32, String)> },
    Craft { recipes: Vec<Recipe<String>> },
}
#[derive(Deserialize, Debug, Clone)]
#[serde(bound(deserialize = ""))]
pub struct PlantAdvancementSum {
    xp_multiplier: f32,
    yield_speed_multiplier: f32,
    yields: Vec<(f32, ArchetypeHandle)>,
    recipes: Vec<Recipe<ArchetypeHandle>>,
}
impl AdvancementSum for PlantAdvancementSum {
    type Kind = PlantAdvancementKind;

    fn new(unlocked: &[Advancement<Self>]) -> Self {
        use PlantAdvancementKind::*;

        let mut sum = PlantAdvancementSum {
            xp_multiplier: 1.0,
            yield_speed_multiplier: 1.0,
            yields: vec![],
            recipes: vec![],
        };

        for k in unlocked.iter() {
            match &k.kind {
                Xp { multiplier } => {
                    sum.xp_multiplier *= multiplier;
                }
                YieldSpeed { multiplier } => {
                    sum.yield_speed_multiplier *= multiplier;
                },
                YieldNeighboringSize { .. } => {},
                Yield { resources } => {
                    sum.yields.append(
                        &mut resources
                            .iter()
                            .map(|(c, s)| Ok((*c, CONFIG.find_possession_handle(s)?)))
                            .collect::<Result<Vec<_>, ConfigError>>()
                            .expect("couldn't find archetype for advancement yield")
                    )
                }
                Craft { recipes } => {
                    sum.recipes.append(
                        &mut recipes
                            .iter()
                            .map(|r| {
                                Ok(Recipe {
                                    makes: CONFIG.find_possession_handle(&r.makes)?,
                                    needs: r.needs.iter().map(|(c, s)| {
                                        Ok((*c, CONFIG.find_possession_handle(s)?))
                                    })
                                    .collect::<Result<Vec<_>, ConfigError>>()?
                                })
                            })
                            .collect::<Result<Vec<_>, ConfigError>>()
                            .expect("couldn't find archetype for crafting advancement")
                    )
                }
            }
        }

        sum
    }
}

pub trait AdvancementSum: DeserializeOwned + fmt::Debug {
    type Kind: DeserializeOwned + fmt::Debug + Clone;

    fn new(unlocked: &[Advancement<Self>]) -> Self;
}

#[derive(Deserialize, Debug, Clone)]
#[serde(bound(deserialize = ""))]
pub struct Advancement<S: AdvancementSum> {
    kind: S::Kind,
    xp: u64,
    title: String,
    description: String,
    achiever_title: String,
}
#[derive(Deserialize, Debug, Clone)]
#[serde(bound(deserialize = ""))]
pub struct AdvancementSet<S: AdvancementSum> {
    base: Advancement<S>,
    rest: Vec<Advancement<S>>
}
impl<S: AdvancementSum> AdvancementSet<S> {
    pub fn all(mut self) -> Vec<Advancement<S>> {
        self.rest.insert(0, self.base);
        self.rest
    }

    pub fn sum(&self, xp: u64) -> S {
        S::new(&self.rest[0..self.current_position(xp)])
    }

    pub fn max(&self) -> S {
        S::new(&self.rest)
    }

    pub fn current(&self, xp: u64) -> &Advancement<S> {
        self.rest.iter().find(|x| x.xp > xp).unwrap_or(&self.base)
    }

    pub fn current_position(&self, xp: u64) -> usize {
        self.rest.iter().position(|x| x.xp > xp).unwrap_or(0)
    }
}


#[test]
/// In the CONFIG, you can specify the names of archetypes.
/// If you're Rishi, you might spell one of those names wrong.
/// This test helps you make sure you didn't do that.
fn archetype_name_matches() {
    for a in CONFIG.possession_archetypes.iter() {
        match &a.kind {
            ArchetypeKind::Seed(sa) => assert!(
                CONFIG.find_plant(&sa.grows_into).is_ok(),
                "seed archetype {:?} claims it grows into unknown plant archetype {:?}",
                a.name,
                sa.grows_into,
            ),
            _ => {}
        }
    }

    for arch in CONFIG.plant_archetypes.iter().cloned() {
        for adv in arch.advancements.all().iter() {
            use PlantAdvancementKind::*;

            match &adv.kind {
                Yield { resources } => {
                    for (_, item_name) in resources.iter() {
                        assert!(
                            CONFIG.find_possession(item_name).is_ok(),
                            "Yield advancement {:?} for plant {:?} includes unknown resource {:?}",
                            adv.title,
                            arch.name,
                            item_name,
                        )
                    }
                }
                Craft { recipes } => {
                    for Recipe { makes, needs } in recipes.iter() {
                        assert!(
                            CONFIG.find_possession(makes).is_ok(),
                            "Crafting advancement {:?} for plant {:?} produces unknown resource {:?}",
                            adv.title,
                            arch.name,
                            makes,
                        );
                        for (_, resource) in needs.iter() {
                            assert!(
                                CONFIG.find_possession(resource).is_ok(),
                                "Crafting advancement {:?} for plant {:?} uses unknown resource {:?} in recipe for {:?}",
                                adv.title,
                                arch.name,
                                resource,
                                makes
                            )
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
