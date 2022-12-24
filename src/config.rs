use twitch_api::types::NicknameRef;

use crate::youtube::api::YoutubeHandleRef;

#[derive(Debug)]
pub struct CreatorNames {
    pub twitch: &'static [&'static twitch_api::types::NicknameRef],
    pub youtube: &'static [&'static YoutubeHandleRef],
}

#[derive(Debug, Clone, Copy)]
pub struct Campaign {
    pub id: u64,
}

#[derive(Debug)]
pub struct Config {
    pub campaign: Campaign,
    pub creators: CreatorNames,
}

pub static CONFIG: Config = Config {
    campaign: Campaign { id: 468510 },

    // TODO: FIXME: User IDs?
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
            NicknameRef::from_str("amouranth"),
            #[cfg(debug_assertions)]
            NicknameRef::from_str("rainbow6"),
            #[cfg(debug_assertions)]
            NicknameRef::from_str("bobross"),
            #[cfg(debug_assertions)]
            NicknameRef::from_str("dusterthefirst"),
        ],
        youtube: &[
            YoutubeHandleRef::from_str("@ReapeeRon"),
            YoutubeHandleRef::from_str("@Santaa."),
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
