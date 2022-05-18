use crate::*;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct UserInfo {
    pub level: u8,
    pub temp_points: u32,
    pub perm_points: u32,
    pub last_updated_at: u64,
}
