use karta_core::parser::RegisteredPattern;
use karta_core::patterns;

pub(super) fn registered_patterns() -> Vec<RegisteredPattern> {
    patterns![
        "bbs_link"           => r"xiaoheihe\.cn/bbs/link/([a-zA-Z0-9]+)",
        "bbs_share"          => r"xiaoheihe\.cn(?:/v\d+)?/bbs/app/api/web/share\?.*?link_id=([a-zA-Z0-9]+)",
        "game_pc"            => r"xiaoheihe\.cn/game/pc/([a-zA-Z0-9]+)",
        "game_console"       => r"xiaoheihe\.cn/game/console/([a-zA-Z0-9]+)",
        "game_mobile"        => r"xiaoheihe\.cn/game/mobile/([a-zA-Z0-9]+)",
        "game_share_pc"      => r"xiaoheihe\.cn/game/share_game_detail\?.*?appid=([a-zA-Z0-9]+).*?game_type=pc",
        "game_share_console" => r"xiaoheihe\.cn/game/share_game_detail\?.*?appid=([a-zA-Z0-9]+).*?game_type=console",
        "game_share_mobile"  => r"xiaoheihe\.cn/game/share_game_detail\?.*?appid=([a-zA-Z0-9]+).*?game_type=mobile",
    ]
}
