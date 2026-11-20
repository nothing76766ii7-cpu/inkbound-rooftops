//! Campaign progression, regions, seasons, weather, and illustrated storyboard text.
//! Pure deterministic data: no file/network access, and no per-frame allocations for world generation.
use glam::Vec3;

pub const CAMPAIGN_LEVELS: u32 = 24;
pub const LEVELS_PER_REGION: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season { Spring, Summer, Autumn, Winter }
impl Season {
    pub fn name(self) -> &'static str {
        match self { Self::Spring => "SPRING", Self::Summer => "SUMMER", Self::Autumn => "AUTUMN", Self::Winter => "WINTER" }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weather { Clear, Mist, Rain, Snow, Wind }
impl Weather {
    pub fn name(self) -> &'static str {
        match self { Self::Clear => "CLEAR", Self::Mist => "MIST", Self::Rain => "RAIN", Self::Snow => "SNOW", Self::Wind => "WIND" }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LevelTheme {
    pub region: &'static str,
    pub title: &'static str,
    pub season: Season,
    pub weather: Weather,
    pub chapter: usize,
    pub act: usize,
    pub difficulty: u8,
}

const REGIONS: [&str; 6] = [
    "INKWELL CANALS", "SAFFRON BAZAAR", "EMBER FOUNDRIES",
    "FROSTGLASS PEAKS", "MONSOON PORT", "ECLIPSE SPIRES",
];
const STAGES: [[&str; 4]; 6] = [
    ["THE FIRST MARK", "CANAL ROOFS", "THE BROKEN BRIDGE", "THE GATE OF INK"],
    ["MARKET OF WIND", "AWNINGS ABOVE", "THE CLOCKKEEPERS", "THE SUNLIT CHASE"],
    ["SMOKE AND STEEL", "THE HOT CHIMNEYS", "THE FRACTURED RAIL", "THE IRON BELFRY"],
    ["PALE HORIZON", "THE ICEBOUND PASS", "WALLS OF FROST", "THE FROZEN BELL"],
    ["RAINSONG DOCKS", "THE TIDAL CRANES", "STORM CROSSING", "THE LOST LIGHTHOUSE"],
    ["CITY ABOVE CLOUDS", "THE LAST BRUSH", "CUT THE CHAINS", "RESTORE THE SKY"],
];

pub fn level_theme(level_number: u32) -> LevelTheme {
    let level = level_number.clamp(1, CAMPAIGN_LEVELS) - 1;
    let chapter = (level / LEVELS_PER_REGION) as usize;
    let act = (level % LEVELS_PER_REGION) as usize;
    let season = match chapter { 0 => Season::Spring, 1 => Season::Summer, 2 => Season::Autumn,
        3 => Season::Winter, 4 => Season::Spring, _ => Season::Winter };
    let weather = match chapter {
        0 => if act == 2 { Weather::Mist } else { Weather::Clear },
        1 => if act == 3 { Weather::Wind } else { Weather::Clear },
        2 => if act == 0 { Weather::Mist } else { Weather::Wind },
        3 => Weather::Snow,
        4 => Weather::Rain,
        _ => if act >= 2 { Weather::Wind } else { Weather::Mist },
    };
    LevelTheme { region: REGIONS[chapter], title: STAGES[chapter][act], season, weather,
        chapter, act, difficulty: (chapter as u8 * 2 + act as u8).min(13) }
}

#[derive(Debug, Clone, Copy)]
pub struct StoryBeat {
    pub heading: &'static str,
    pub line1: &'static str,
    pub line2: &'static str,
}

/// Three playable-between-level storyboard pages per chapter: illustrated by the UI renderer.
const STORY: [[StoryBeat; 3]; 6] = [
    [
        StoryBeat { heading: "CHAPTER I / LOST INK", line1: "THE SKY HAS BEEN SEWN SHUT.", line2: "ONLY YOUR BRUSH CAN OPEN IT." },
        StoryBeat { heading: "THE FIRST MARK", line1: "FOLLOW THE TEAL BEACONS.", line2: "LEARN TO FLY BETWEEN ROOFS." },
        StoryBeat { heading: "THE HIDDEN SEAM", line1: "A SWORD FIGHTS THE SHADOWS.", line2: "THE RIFT CUTS THE CITY ITSELF." },
    ],
    [
        StoryBeat { heading: "CHAPTER II / THE MARKET", line1: "SUNLIGHT HIDES OLD SECRETS.", line2: "THE MAP IS WRITTEN ON ROOFS." },
        StoryBeat { heading: "THE WIND CHASE", line1: "GRAPPLE HIGHER THAN YOU JUMP.", line2: "CHAIN THE SWINGS. STAY MOVING." },
        StoryBeat { heading: "THE CLOCKKEEPER", line1: "THE BELLS ARE COUNTING DOWN.", line2: "REACH THE HIGHEST GATE." },
    ],
    [
        StoryBeat { heading: "CHAPTER III / ASH", line1: "THE FOUNDRIES BURN THROUGH FOG.", line2: "THE CITY IS COMING APART." },
        StoryBeat { heading: "BENEATH THE SMOKE", line1: "THE WRAITHS GUARD THE CHAINS.", line2: "FIGHT THEM WITHOUT BREAKING PATHS." },
        StoryBeat { heading: "THE IRON BELFRY", line1: "CUT THE ANCHOR WITH THE RIFT.", line2: "KEEP THE BRUSH FOR THE LEAP." },
    ],
    [
        StoryBeat { heading: "CHAPTER IV / WHITEOUT", line1: "WINTER SWALLOWS THE SUMMITS.", line2: "EVEN THE INK RUNS SLOW." },
        StoryBeat { heading: "THE FROZEN PASS", line1: "TRUST YOUR FEET AND YOUR WEB.", line2: "THE NEXT LEDGE IS FAR AWAY." },
        StoryBeat { heading: "BELL IN THE SNOW", line1: "ONE LAST LIGHT STILL BURNS.", line2: "FOLLOW ITS RING TO THE SEA." },
    ],
    [
        StoryBeat { heading: "CHAPTER V / MONSOON", line1: "RAIN WASHES THE MAP AWAY.", line2: "PAINT A NEW ROUTE ABOVE WAVES." },
        StoryBeat { heading: "THE TIDAL CRANES", line1: "SWING PAST THE BROKEN PORT.", line2: "DO NOT LOSE YOUR MOMENTUM." },
        StoryBeat { heading: "THE LOST BEACON", line1: "THE LIGHTHOUSE POINTS UPWARD.", line2: "THE LAST CITY WAITS IN CLOUDS." },
    ],
    [
        StoryBeat { heading: "CHAPTER VI / ECLIPSE", line1: "THE SKY IS SPLIT BY CHAINS.", line2: "THE LAST ASCENT BEGINS NOW." },
        StoryBeat { heading: "THE FINAL STROKE", line1: "FIGHT THE WRAITHS WITH STEEL.", line2: "CUT ONLY WITH THE RIFT." },
        StoryBeat { heading: "RESTORE THE SKY", line1: "BRUSH AND BLADE WORK AS ONE.", line2: "THE CITY REMEMBERS THE SUN." },
    ],
];

pub fn story_beat(level_number: u32, page: usize) -> StoryBeat {
    STORY[level_theme(level_number).chapter][page.min(2)]
}

#[derive(Debug, Clone, Copy)]
pub struct Atmosphere {
    pub daylight: f32,
    pub sun_direction: Vec3,
    pub sky: [f32; 3],
    pub fog: [f32; 3],
    pub tint: [f32; 3],
    pub fog_near: f32,
    pub fog_range: f32,
    pub weather: Weather,
}

/// 0.0 = midnight, 0.25 = sunrise, 0.5 = midday, 0.75 = sunset.
pub fn atmosphere(level_number: u32, day_phase: f32) -> Atmosphere {
    let theme = level_theme(level_number);
    let sun_angle = (day_phase - 0.25) * std::f32::consts::TAU;
    let height = sun_angle.sin();
    let daylight = ((height + 0.25) * 0.9).clamp(0.17, 1.0);
    let sun_direction = Vec3::new(sun_angle.cos() * 0.65, height.max(0.12), 0.40).normalize();
    let (sky_day, fog_day, tint) = match theme.chapter {
        0 => ([0.56, 0.71, 0.77], [0.64, 0.72, 0.72], [0.93, 1.03, 0.98]),
        1 => ([0.77, 0.67, 0.56], [0.80, 0.70, 0.61], [1.08, 0.97, 0.84]),
        2 => ([0.61, 0.53, 0.51], [0.70, 0.59, 0.55], [1.03, 0.91, 0.83]),
        3 => ([0.61, 0.70, 0.83], [0.73, 0.79, 0.87], [0.91, 0.98, 1.12]),
        4 => ([0.40, 0.56, 0.65], [0.50, 0.63, 0.70], [0.89, 0.98, 1.05]),
        _ => ([0.43, 0.40, 0.56], [0.56, 0.53, 0.63], [0.93, 0.87, 1.07]),
    };
    let night_sky = [0.075, 0.095, 0.16];
    let night_fog = [0.14, 0.17, 0.23];
    let mix = |day: [f32;3], night: [f32;3]| -> [f32;3] {
        [night[0] + (day[0]-night[0])*daylight,
         night[1] + (day[1]-night[1])*daylight,
         night[2] + (day[2]-night[2])*daylight]
    };
    Atmosphere { daylight, sun_direction, sky: mix(sky_day, night_sky),
        fog: mix(fog_day, night_fog), tint,
        fog_near: if matches!(theme.weather, Weather::Mist | Weather::Snow) { 13.0 } else { 24.0 },
        fog_range: if matches!(theme.weather, Weather::Mist | Weather::Snow | Weather::Rain) { 70.0 } else { 115.0 },
        weather: theme.weather }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_campaign_entries_have_identity_and_story() {
        for level in 1..=CAMPAIGN_LEVELS {
            let stage = level_theme(level);
            assert!(!stage.title.is_empty());
            assert!(!stage.region.is_empty());
            assert!(!story_beat(level, 2).line2.is_empty());
        }
    }
    #[test]
    fn day_and_night_use_different_sun_and_sky() {
        let noon = atmosphere(2, 0.5);
        let midnight = atmosphere(2, 0.0);
        assert!(noon.daylight > midnight.daylight);
        assert!(noon.sky[0] > midnight.sky[0]);
    }
}
