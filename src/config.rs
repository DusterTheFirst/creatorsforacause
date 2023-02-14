use std::time::Duration;

use twitch_api::types::NicknameRef;

use crate::watcher::youtube::api::YoutubeHandleRef;

#[derive(Debug)]
pub struct CreatorNames {
    pub twitch: &'static [&'static twitch_api::types::NicknameRef],
    pub youtube: &'static [&'static YoutubeHandleRef],
}

#[derive(Debug, Clone, Copy)]
pub struct CampaignConfig {
    pub id: u32,
}

#[derive(Debug)]
pub struct Config {
    pub campaign: CampaignConfig,
    pub creators: CreatorNames,
    pub refresh_period: Duration,
}

pub static CONFIG: Config = Config {
    refresh_period: Duration::from_secs(10 * 60), // 10 minutes

    campaign: CampaignConfig { id: 468510 },

    // TODO: FIXME: User IDs!!!!!!!!
    creators: CreatorNames {
        twitch: &[
            NicknameRef::from_str("gathe_"),
            // NicknameRef::from_str("inkierain"),
            NicknameRef::from_str("kkywi"),
            NicknameRef::from_str("megapigstep"),
            NicknameRef::from_str("tridegd"),
            //
            // Test accounts
            #[cfg(debug_assertions)]
            NicknameRef::from_str("linustech"),
            #[cfg(debug_assertions)]
            NicknameRef::from_str("xqc"),
            #[cfg(debug_assertions)]
            NicknameRef::from_str("loltyler1"),
            #[cfg(debug_assertions)]
            NicknameRef::from_str("summit1g"),
            #[cfg(debug_assertions)]
            NicknameRef::from_str("rainbow6"),
            #[cfg(debug_assertions)]
            NicknameRef::from_str("bobross"),
            #[cfg(debug_assertions)]
            NicknameRef::from_str("dusterthefirst"),
        ],
        youtube: &[
            YoutubeHandleRef::from_str("@ReapeeRon"),
            YoutubeHandleRef::from_str("@santaagd"),
            //
            // Test accountts
            #[cfg(debug_assertions)]
            YoutubeHandleRef::from_str("@LofiGirl"),
            #[cfg(debug_assertions)]
            YoutubeHandleRef::from_str("@dusterthefirst"),
            #[cfg(debug_assertions)]
            YoutubeHandleRef::from_str("@therealgathe"),
            #[cfg(debug_assertions)]
            YoutubeHandleRef::from_str("@ludwig"),
            #[cfg(debug_assertions)]
            YoutubeHandleRef::from_str("@jaidenanimations"),
        ],
    },
};
