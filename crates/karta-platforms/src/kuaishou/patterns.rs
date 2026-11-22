use karta_core::parser::RegisteredPattern;
use karta_core::patterns;

pub(super) fn registered_patterns() -> Vec<RegisteredPattern> {
    patterns![
        "v_kuaishou"    => r"v\.kuaishou\.com/[A-Za-z\d._?%&+\-=/#]+",
        "kuaishou"      => r"(?:www\.)?kuaishou\.com/[A-Za-z\d._?%&+\-=/#]+",
        "chenzhongtech" => r"(?:v\.m\.)?chenzhongtech\.com/fw/[A-Za-z\d._?%&+\-=/#]+",
    ]
}
