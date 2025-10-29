//! Configuration for the Xiangqi engine.

pub struct Config {
    // Evaluation constants
    pub bonus_bottom_cannon: i32,
    pub bonus_palace_heart_horse: i32,
    pub king_safety_penalty_per_guard: i32,
    pub dynamic_bonus_attack_per_missing_defender: i32,
    pub mobility_bonus_rook: i32,
    pub mobility_bonus_horse: i32,
    pub mobility_bonus_cannon: i32,

    // Search constants
    pub lmr_reduction: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bonus_bottom_cannon: 80,
            bonus_palace_heart_horse: 70,
            king_safety_penalty_per_guard: 50,
            dynamic_bonus_attack_per_missing_defender: 15,
            mobility_bonus_rook: 1,
            mobility_bonus_horse: 3,
            mobility_bonus_cannon: 1,
            lmr_reduction: 1,
        }
    }
}
