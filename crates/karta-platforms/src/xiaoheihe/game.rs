const API_GAME_PC: &str = "https://api.xiaoheihe.cn/game/get_game_detail";
const API_GAME_CONSOLE: &str = "https://api.xiaoheihe.cn/game/console/get_game_detail";
const API_GAME_MOBILE: &str = "https://api.xiaoheihe.cn/game/mobile/get_game_detail";

#[derive(Debug, Clone, Copy)]
pub(super) enum GameType {
    Pc,
    Console,
    Mobile,
}

impl GameType {
    pub(super) fn api_url(&self) -> &'static str {
        match self {
            GameType::Pc => API_GAME_PC,
            GameType::Console => API_GAME_CONSOLE,
            GameType::Mobile => API_GAME_MOBILE,
        }
    }

    pub(super) fn api_path(&self) -> &'static str {
        match self {
            GameType::Pc => "game/get_game_detail",
            GameType::Console => "game/console/get_game_detail",
            GameType::Mobile => "game/mobile/get_game_detail",
        }
    }

    pub(super) fn id_param_name(&self) -> &'static str {
        match self {
            GameType::Pc => "steam_appid",
            GameType::Console | GameType::Mobile => "appid",
        }
    }
}
