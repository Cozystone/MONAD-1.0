//! 媛곸꽦 猷⑦봽 (Wake) ??B2 ?뺤갑 쨌 B3 ?대줎 ?깆옣 쨌 B4 怨꾪쉷 쨌 B5 二쇱쓽.
//!
//! PRD 짠4.7???⑥씪 猷⑦봽. ?덈젴 ?ㅽ겕由쏀듃媛 ?녿떎. 泥?遺?낅????먭린源뚯? ?닿쾬留??덈떎.
//!
//! ```text
//! o ??sense()                       ?대깽??援щ룞: 諛붾?寃껊쭔
//! x ??encode(o)                     ?몄? ?먯옄濡?//! s ??settle(x, s_prev, ???         F 理쒖냼?? 吏媛?= 異붾줎
//! if ?붿뿬 F > 罐: s ???대줎 遺꾪솕        ?ㅻ챸 ?ㅽ뙣 = 諛곗슱 寃?(1-shot 援ъ“ ?깆옣)
//! ? ??plan(s, C, ???                G 理쒖냼?? 濡ㅼ븘??= ?곗긽 泥댁씠??//! act(?.first); write_counts(...)   援?냼 移댁슫??媛깆떊 ???숈뒿???꾨?
//! ```
//!
//! # ?섎굹??紐⑹쟻?⑥닔
//!
//! - **吏媛겶룻븰??*? ?먯쑀?먮꼫吏 F瑜?以꾩씤?? 愿痢≪씠 ?ㅻ챸?섏? ?딆쑝硫??붿뿬 F媛 ?щ㈃)
//!   洹멸쾬??怨??숈뒿 ?좏샇?? ?숈뒿瑜좊룄 ?먰룺???녿떎.
//! - **?됰룞**? 湲곕??먯쑀?먮꼫吏 G瑜?以꾩씤?? ?ㅼ슜 媛移??좏샇 ?꾨떖) + ?뺣낫 ?대뱷(遺덊솗?ㅽ븳
//!   怨??먯깋). **?멸린?ъ씠 蹂꾨룄 紐⑤뱢???꾨땲??紐⑹쟻?⑥닔??????*?대떎.
//!
//! # System 1 / System 2
//!
//! ?섏? ?ㅻⅨ 紐⑤뜽???꾨땲??**媛숈? 猷⑦봽??諛섎났 ?덉궛 李⑥씠**?? ?덉궛??以꾩씠硫?諛섏궗,
//! ?섎━硫??숆퀬. ?쒓컙???놁쑝硫??뺢쾶, ?덉쑝硫?源딄쾶 ??anytime ?쒖뒪??PRD 짠4.4).

use crate::atom::Val;
use crate::encode::{Encoder, Obs};
use crate::graph::WorldGraph;
use crate::rng::Rng;
use crate::sbv::{Bundler, Sbv};
use crate::store::Store;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// 吏媛곷떦 ?대줎 ?곹븳.
///
/// ?됰꼮?섍쾶 ?〓뒗?? **媛곸꽦? ?됰꼮??媛덈씪?먭퀬, ?섎㈃(C1 BMR)???⑹튇??*??寃껋씠
/// PRD??遺꾩뾽?닿린 ?뚮Ц?대떎. ?ш린???꾨겮硫??쒕줈 ?ㅻⅨ ?곹솴?????대줎?쇰줈 萸됯컻??/// ?멸퀎 紐⑤뜽??臾대꼫吏꾨떎 ??諛섎?濡??됰꼮???댁뼱?먮㈃ ??퉬???섎㈃???뚯닔?쒕떎.
pub const MAX_CLONES_PER_PERCEPT: usize = 2048;

/// 臾몃㎘ 踰≫꽣???대뒗 怨쇨굅 嫄몄쓬 ??
pub const DEFAULT_CONTEXT_ORDER: usize = 2;

#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// 愿痢↔낵 吏媛??먰삎??媛숇떎怨?蹂?理쒕? 嫄곕━.
    pub percept_tol: u32,
    /// 吏媛곷떦 ?대줎 ?곹븳.
    pub max_clones: usize,
    /// ?꾩씠 ?뺣쪧???붾━?대젅 ?됲솢.
    pub alpha: f32,
    /// 愿痢?遺덉씪移섎? F??諛섏쁺?섎뒗 媛以묒튂.
    pub obs_weight: f32,
    /// 怨꾪쉷: ?뺣낫 ?대뱷 ??쓽 媛以묒튂(?멸린?ъ쓽 ?멸린).
    pub info_weight: f32,
    /// 怨꾪쉷: ?꾩씠 遺덊솗?ㅼ꽦?????踰뚯젏.
    pub uncertainty_weight: f32,
    /// 怨꾪쉷 ?덉궛(?몃뱶 ?뺤옣 ?잛닔) = ???
    pub plan_budget: usize,
    /// 怨꾪쉷 理쒕? 源딆씠.
    pub plan_depth: usize,
    /// ???붿뿬 F瑜??섏쑝硫?"?ㅻ챸 ?ㅽ뙣"濡?蹂닿퀬 援ъ“瑜??섎┛??
    ///
    /// 湲곕낯媛믪? 臾댄븳??? **媛?ν븳 ?ㅻ챸???섎굹???놁쓣 ?뚮쭔** 援ъ“瑜??섎┛??
    /// ?좏븳??媛믪쓣 二쇰㈃ "洹몃윺??븯吏 ?딅떎"???댁쑀濡쒕룄 ?대줎??留뚮뱾????컻?쒕떎.
    pub surprise_threshold: f32,
    /// 臾몃㎘ 踰≫꽣???대뒗 怨쇨굅 嫄몄쓬 ??0?대㈃ 臾몃㎘ ?놁쓬 = 吏媛곷쭔?쇰줈 ?곹깭 寃곗젙).
    pub context_order: usize,
    /// 臾몃㎘ 踰≫꽣瑜?媛숇떎怨?蹂?理쒕? 嫄곕━.
    pub context_tol: u32,
    /// 吏???섏? 臾몃㎘ 異붾줎(M1 ?ㅽ뿕 湲곕뒫, 湲곕낯 爰쇱쭚).
    ///
    /// 耳쒕㈃ "?닿? ?대뒓 ?멸퀎???덈뒗媛"瑜?沅ㅼ쟻 ?앹〈?⑤줈 異붾줎??吏?꾨? ?꾪솚쨌媛쒖꽕?쒕떎.
    /// 媛먯?湲걔룹쓳怨?二쇨린쨌蹂묓빀 洹쒖튃???곹샇?묒슜??誘명빐寃곗씠??LAB-NOTEBOOK??10?섍꼍
    /// 諛섎났 湲곕줉 李몄“) ?⑥씪 ?섍꼍 ?먮쫫?먯꽌??爰??먯뼱???쒕떎.
    pub map_inference: bool,
    /// ?뺤갑 ?덉궛 ???= ?숈떆???좎??섎뒗 ?곹깭 媛????鍮???.
    ///
    /// 1?대㈃ 留????섎굹濡??⑥젙?섎뒗 ?먯슃 ?꾪꽣媛 ?쒕떎 ??蹂꾩묶 ?멸퀎?먯꽌 ??踰??룰컝由щ㈃
    /// 蹂듦뎄??湲몄씠 ?녿떎. ?щ읉???대젮?먮㈃ ?ㅼ쓬 愿痢〓뱾????덈줈 媛?ㅻ궦??
    pub beam_width: usize,
    /// 계획: 직전 행동과 다른 첫 행동에 대한 벌점(운동 부드러움 사전분포).
    ///
    /// 0이면 꺼짐(기본 — 미로·게이트 무영향). Pong류 연속 제어에서 EFE 롤아웃의
    /// 동률 잡음이 좌우 떨림(정상상태 침하의 원인)을 만들 때, 소액 전환 비용이
    /// 계획을 관성 있게 만들어 떨림을 없앤다.
    pub switch_cost: f32,
    /// ?ㅻ챸 ?ㅽ뙣媛 ?대쭔???곗냽?섏뼱??"?뺣쭚 ?덈줈???곸뿭"?쇰줈 蹂닿퀬 援ъ“瑜??섎┛??
    ///
    /// ?좉퉸???쇰?(?먰뵾?뚮뱶 ?쒖옉, 轅?吏곹썑)? ?ъ젙?꾨줈 ??댁빞 ?쒕떎. ?ㅽ뙣 ??踰덉뿉
    /// ?대줎??留뚮뱾硫? ?섎㈃???몄슫 源⑤걮??吏?꾨? 媛곸꽦???꾨줈 ?ㅼ뿼?쒗궓??
    pub lost_patience: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            percept_tol: 32,
            max_clones: MAX_CLONES_PER_PERCEPT,
            alpha: 0.5,
            obs_weight: 4.0,
            info_weight: 0.5,
            uncertainty_weight: 1.0,
            plan_budget: 2000,
            plan_depth: 24,
            surprise_threshold: f32::INFINITY,
            context_order: DEFAULT_CONTEXT_ORDER,
            context_tol: 8,
            map_inference: false,
            beam_width: 8,
            switch_cost: 0.0,
            lost_patience: 2,
        }
    }
}

/// ?먰뵾?뚮뱶 湲곗뼲????嫄몄쓬.
#[derive(Clone, Copy, Debug)]
pub struct EpStep {
    pub percept: u32,
    /// ??吏媛곸뿉 ?꾨떖?섍쾶 ???됰룞.
    pub action: u16,
    pub val: Option<Val>,
    /// 洹??쒓컙 媛곸꽦???뺤갑?덈뜕 ?곹깭(?놁쑝硫?u32::MAX).
    ///
    /// ?묎퀬??轅덉쓽 **?뺣젹 蹂묓빀**???대떎: EM???ш뎄?깊븳 ?곹깭媛 媛곸꽦???대뒓 ?쇱쭏
    /// ?곹깭? 媛숈? ?쒓컙?ㅼ쓣 ?댁븯?붿?濡??섏쓣 吏앹??? ??援ъ“瑜?留뚮뱾吏 ?딄퀬 湲곗〈
    /// 援ъ“??利앷굅瑜??≪닔?쒗궓??
    pub state: u32,
}

/// ???깆쓽 ?뺤갑 寃곌낵 ???꾨? ?щ엺???쎌쓣 ???덈떎(?좊━?곸옄 ?섎Т).
#[derive(Clone, Copy, Debug)]
pub struct Settled {
    pub state: u32,
    pub percept: u32,
    /// ?붿뿬 ?먯쑀?먮꼫吏(鍮꾪듃). ?댁닔濡?"?ㅻ챸?섏? 紐삵뻽??.
    pub residual_f: f32,
    /// ???깆뿉 ???대줎??留뚮뱾?덈뒗媛(援ъ“ ?숈뒿???쇱뼱?щ뒗媛).
    pub grew: bool,
    /// ??吏媛곸쓣 泥섏쓬 遊ㅻ뒗媛.
    pub novel_percept: bool,
    /// 怨좊젮???꾨낫 ?곹깭 ??二쇱쓽 ?덉궛???ㅼ륫移?.
    pub considered: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub ticks: u64,
    pub clones_grown: u64,
    pub novel_percepts: u64,
    pub surprise_sum: f64,
    pub plan_expansions: u64,
    pub map_switches: u64,
}

pub struct Agent {
    pub graph: WorldGraph,
    pub encoder: Encoder,
    pub cfg: Config,
    pub stats: Stats,
    /// ?꾩옱 ?곹깭 = 誘우쓬 遺꾪룷??理쒕퉰媛?遺??吏곹썑???놁쓬).
    pub state: Option<u32>,
    /// ?곹깭?????誘우쓬: (?곹깭, 濡쒓렇?뺣쪧). ?뺢퇋?붾릺???덈떎.
    ///
    /// ?⑥씪 ?곹깭媛 ?꾨땲??遺꾪룷瑜??좊떎??寃껋씠 ?듭떖?대떎. 蹂꾩묶 ?멸퀎?먯꽌 吏湲??대뵒?몄???    /// **吏湲?愿痢〓쭔?쇰줈 ?뺥빐吏吏 ?딅뒗??* ???щ윭 媛?μ꽦???대젮?먭퀬 ?댄썑 愿痢≪씠
    /// 媛?ㅻ궡寃??댁빞 ?쒕떎. ?닿쾬???뺤갑??'?좏깮'???꾨땲??'異붾줎'?쇰줈 留뚮뱺??
    pub belief: Vec<(u32, f32)>,
    /// 理쒓렐 ?대젰 (吏媛? 洹?吏곸쟾?????됰룞). 理쒖떊????
    hist: Vec<(u32, u16)>,
    /// ?먰뵾?뚮뱶 湲곗뼲: 寃れ? 洹몃?濡쒖쓽 (吏媛? ?됰룞, ?곗냽?? ?먮쫫.
    ///
    /// 由ы뵆?덉씠 踰꾪띁媛 ?꾨땲????寃쎌궗 ?숈뒿???곗씠吏 ?딅뒗?? ?섎㈃(C1)???꾩뿭 異붾줎?쇰줈
    /// 吏?꾨? ?ㅼ떆 ?몄슱 ???뱀쓣 ?먮즺?? ?대쭏媛 ??쓽 ?쇳솕瑜??ъ깮?섎ŉ ?쇱쭏 吏?꾨?
    /// 援논엳??寃껉낵 媛숈? ??븷?대ŉ, PRD 짠4.5 "?뺤텞??怨?異붿긽?????낅젰???닿쾬?대떎.
    pub episodes: Vec<Vec<EpStep>>,
    /// 臾몃㎘ 踰≫꽣???곗긽 ?됱씤(id = 臾몃㎘ 踰덊샇). ???곹깭媛 ?щ윭 臾몃㎘?쇰줈 ?꾨떖????    /// ?덉쑝誘濡??곹깭 踰덊샇媛 ?꾨땲??臾몃㎘ 踰덊샇濡??됱씤?쒕떎.
    /// 지도별 해마 색인 — 전 지도 공용이면 남의 살아있는 등록이 내 차근접을
    /// 가려(그림자 납치) 복귀마다 클론이 신설된다. 상태가 세계에 속하듯
    /// 주소록도 세계에 속한다.
    ctx: Vec<Store>,
    /// 臾몃㎘ 踰덊샇 ??洹몃븣 諛곗젙???곹깭.
    ctx_node: Vec<Vec<u32>>,
    /// 吏媛곷퀎 ?ㅻ챸 ?ㅽ뙣 移댁슫??
    ///
    /// ?꾩뿭 ?곗냽 移댁슫?곕줈?????쒕떎: 蹂꾩묶 吏媛곸쓽 ?ㅽ뙣??留?諛⑸Ц留덈떎 諛섎났?섏?留?    /// 洹??ъ씠 ?ㅻⅨ 吏媛곷뱾???ㅻ챸?섎ŉ 移댁슫?곕? ?딅뒗?? "??吏媛곸씠 ?먭씀 ?ㅻ챸??    /// ?ㅽ뙣?쒕떎"??吏媛??⑥쐞???ъ떎?대?濡?吏媛??⑥쐞濡??쇰떎.
    fail_count: HashMap<u32, u32>,
    /// **異쒕컻 吏媛곷퀎** ?덉륫 遺덈웾 移댁슫????蹂꾩묶??吏꾩쭨 踰붿씤 異붿쟻.
    ///
    /// 蹂꾩묶??利앹긽? ?꾩갑 吏媛곸쓽 ?섏걶 ?ㅻ챸(?꾩씠 ?뺣쪧??媛덈씪吏??쇰줈 ?섑??섏?留?
    /// 踰붿씤? 異쒕컻 ?곹깭?? ?쒕줈 ?ㅻⅨ ???곹솴?????곹깭濡?萸됱퀜?볦븯湲??뚮Ц??嫄곌린??    /// ?섍???寃곌낵媛 媛덈씪吏꾨떎. 洹몃옒??遺덈웾 ?덉륫? 異쒕컻 吏媛곸뿉 ?곷┰?섍퀬,
    /// ?섏튇 吏媛곸? ?ㅼ쓬 諛⑸Ц ??臾몃㎘ ?대줎?쇰줈 媛뺤젣 遺꾪솕?쒕떎. ?꾩갑 吏媛곸쓣 媛덈씪遊먯빞
    /// ?됰슧??怨노쭔 履쇨컻吏꾨떎(?붾쾭源낆쑝濡??뺤씤???ъ떎).
    fail_source: HashMap<u32, u32>,
    // ---- 吏???섏? 臾몃㎘ (M1 怨꾩링) ----
    //
    // 媛숈? 吏媛??댄쐶瑜??곕뒗 ?щ윭 ?멸퀎瑜?寃る뒗 ?쒓컙 "吏湲?蹂댁씠??寃?留뚯쑝濡쒕뒗 遺議깊븯怨?    // "?닿? ?대뒓 ?멸퀎???덈뒗媛"?쇰뒗 ?곸쐞 ?좎옱蹂?섍? ?꾩슂?섎떎. ?닿쾬???놁쑝硫????멸퀎??    // ?꾩씠媛 ???멸퀎??援ъ“??湲곕줉?쒕떎(?쇱쭏 ?ㅼ뿼 ??10?섍꼍 遺뺢눼??理쒖쥌 ?먯씤).
    /// 吏????
    pub n_maps: u32,
    /// ?쒖꽦 吏??
    pub active_map: u32,
    /// ?몃뱶 ???뚯냽 吏??(洹몃옒?꾩? ?됲뻾, apply_node_map???④퍡 ??릿??.
    pub node_map: Vec<u32>,
    /// ?먰뵾?뚮뱶 ??洹??먰뵾?뚮뱶媛 ?랁븳 吏??(?묎퀬??轅덉씠 吏?꾨퀎濡??섎닠 袁쇰떎).
    pub episode_maps: Vec<u32>,
    /// 理쒓렐 (吏媛? ?됰룞) 沅ㅼ쟻 ??吏???ъ젙?꾩쓽 ?뺥빀??寃?ъ뿉 ?대떎.
    trail: Vec<(u32, u16)>,
    /// 理쒓렐 ?깆쓽 ?ㅻ챸 ?덉쭏(true=?묓샇) ???꾩뿭 ?ㅽ뙣?⑤줈 "?멸퀎媛 諛붾뚯뿀??瑜?媛먯?.
    recent_quality: Vec<bool>,
    /// 吏???먭? ?ъ씠??理쒖냼 媛꾧꺽 移댁슫??
    since_map_check: u32,
    /// 吏?꾨퀎 媛쒖꽕 ?쒖젏(?? ??媛??쒖뼱??吏?꾨뒗 ?ъ젙???먮떒?먯꽌 蹂댄샇?쒕떎.
    pub(crate) map_birth: Vec<u64>,
    /// ?곗냽 ??앹〈???잛닔 ????吏??媛쒖꽕???댁쨷 ?뺤씤.
    low_streak: u32,
    /// **?뚰봽??吏???쇳빀**: 吏???ы썑?뺣쪧 P(m). 寃쎌꽦 ?꾪솚???ㅽ뙋 鍮꾩슜 鍮꾨?移?    /// (??踰덉쓽 ?ㅽ뙋??吏???섎굹瑜??듭㎏濡???퉬)???놁븷湲??꾪빐, 留??깆쓽 ?꾩씠 ?ㅻ챸
    /// ?곕룄濡??곗냽 媛깆떊?쒕떎 ???ㅽ뙋???ㅼ쓬 愿痢〓뱾濡??먭린?섏젙?쒕떎.
    pub(crate) map_post: Vec<f32>,
    /// 誘몄????멸퀎 媛??m*???뺣쪧 吏덈웾 ???꾧퀎瑜??섏쑝硫???吏?꾨줈 ?ㅼ껜?뷀븳??
    pub p_new: f32,
    /// 吏?꾨퀎 誘몃땲 誘우쓬 泥댁씤(??돞3) ???곕룄???먯쿇.
    ///
    /// ?꾩씠 ?④굔 ?곕룄???먮퀎?μ씠 ?녿떎(?깆닕 吏?꾨뒗 ?몃? ?꾩씠??吏媛??섏??먯꽌
    /// 40~80% ?곗뿰 ?ㅻ챸 ???쒕룄 5쨌33??怨듯넻 ?ъ씤). ?먮퀎?μ? ?ъ뒳 ?쒖빟?먯꽌 ?섏삩??
    /// 媛?吏???덉뿉???묒? 誘우쓬???ㅼ젣濡?援대━怨? **泥댁씤???댁븘?⑤뒗媛**瑜??곕룄濡??대떎.
    pub(crate) map_chain: Vec<Vec<u32>>,
    /// 지도별 피질(응고) 노드 수 — 신생 보호의 올바른 기준.
    ///
    /// 시간(3,000틱) 기준 보호는 첫 밤(9,000틱) 전에 만료되어, 신생 지도가 자기
    /// 피질이 생기기도 전에 우도 경쟁에서 옛 지도의 우연 생존율에 argmax를
    /// 빼앗긴다(소프트 대역 정체의 마지막 구멍). 보호는 "잠들 때까지"가 맞다.
    pub map_cortical: Vec<u32>,
    /// 諛??묎퀬쨌??? 吏곹썑??媛먯? ?좎삁 ?????좎뿉??源?吏곹썑???ъ젙李??뚯쓬??    /// "?멸퀎媛 諛붾뚯뿀??濡??ㅽ뙋?섏? ?딅뒗??媛먯?湲??ㅻ컻???ㅼ륫 ?먯씤).
    pub(crate) map_check_grace: u32,
    /// 留덉?留??묎퀬???댄썑 媛곸꽦??留뚮뱺 ?대줎????**?대쭏 ?붿쟻**.
    ///
    /// 媛곸꽦??1-shot ?대줎? ?꾩떆 洹쇱궗?? ?묎퀬??轅덉씠 媛숈? 寃쏀뿕?먯꽌 源⑤걮??援ъ“瑜?    /// 戮묒븘 蹂묓빀?섍퀬 ?섎㈃ ???붿쟻?ㅼ? ??댁떆?⑤떎. ?④꺼?먮㈃ 源⑤걮??援ъ“? ?섎???    /// 寃쎌웳?섎ŉ ?뺤갑???⑹튂?쒕떎(10?섍꼍 ?곗냽 ?숈뒿 遺뺢눼??理쒖쥌 ?먯씤?쇰줈 ?ㅼ륫 ?뺤씤).
    pub(crate) fresh_clones: Vec<u32>,
    /// 理쒓렐 諛잛? (?곹깭, ?됰룞) ??怨꾪쉷???쒖옄由ш구??諛⑹?(?遺).
    ///
    /// 吏?꾩뿉 以묐났 ?대줎???⑥븘 媛꾩꽑??遺꾩궛?섎㈃ "A?먯꽌??B濡?媛??寃?理쒖꽑, B?먯꽌??    /// A濡?媛??寃?理쒖꽑"??援?냼 ?⑥젙???앷릿?? 諛⑷툑 ??湲몄쓣 ?좎떆 鍮꾩떥寃?留ㅺ린硫?    /// ?⑥젙?먯꽌 ?ㅼ뒪濡?嫄몄뼱?섏삩?????щ엺??"?꾧퉴 媛遊ㅼ옏?????대떦?쒕떎.
    recent: Vec<(u32, u16)>,
    /// ?좏샇: 蹂닿퀬 ?띠? 吏媛?/ ?덇퀬 ?띠? ?곹깭. ?λ룞異붾줎??C 踰≫꽣.
    pref_percept: HashMap<u32, f32>,
    pref_state: HashMap<u32, f32>,
    // ---- 지속성: 에피소드 저널 + 지도 상태 스냅숏 (M1 마감) ----
    // 그래프는 저널로 kill -9 안전하지만 에피소드 버퍼(꿈 원료)는 RAM에만 있었다.
    // 각성 걸음을 저널에 이어 쓰고, 밤의 끝(은퇴)에 지도 상태 스냅숏과 함께
    // 저널을 비운다. 복구 = 그래프 attach + agent.snap + episodes.journal 재생.
    ep_dir: Option<std::path::PathBuf>,
    ep_journal: Option<std::io::BufWriter<std::fs::File>>,
    ep_dirty: u32,
    /// 직전에 실행된 행동 — 계획 전환 비용(switch_cost)의 기준점.
    last_action: u16,
}

impl Agent {
    pub fn new() -> Self {
        Agent::with_config(Config::default())
    }

    pub fn with_config(cfg: Config) -> Self {
        Agent {
            graph: WorldGraph::new(),
            encoder: Encoder::new(),
            cfg,
            stats: Stats::default(),
            state: None,
            belief: Vec::new(),
            hist: Vec::new(),
            episodes: vec![Vec::new()],
            ctx: vec![Store::new()],
            ctx_node: vec![Vec::new()],
            fail_count: HashMap::new(),
            fail_source: HashMap::new(),
            n_maps: 1,
            active_map: 0,
            node_map: Vec::new(),
            episode_maps: vec![0],
            trail: Vec::new(),
            recent_quality: Vec::new(),
            since_map_check: 0,
            map_birth: vec![0],
            low_streak: 0,
            map_post: vec![1.0],
            p_new: 0.02,
            map_chain: vec![Vec::new()],
            map_cortical: vec![0],
            map_check_grace: 0,
            fresh_clones: Vec::new(),
            recent: Vec::new(),
            pref_percept: HashMap::new(),
            pref_state: HashMap::new(),
            ep_dir: None,
            ep_journal: None,
            ep_dirty: 0,
            last_action: u16::MAX,
        }
    }

    pub fn attach(dir: impl AsRef<std::path::Path>, cfg: Config) -> std::io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let mut a = Agent::with_config(cfg);
        a.graph = WorldGraph::attach(&dir)?;
        // 지도 상태 스냅숏(밤의 끝마다 갱신) — 없으면 단일 지도에서 출발
        let snap = dir.join("agent.snap");
        if snap.exists() {
            a.load_agent_snap(&snap)?;
        } else {
            a.node_map = vec![0; a.graph.n_nodes()];
        }
        // 에피소드 저널 재생 — 마지막 밤 이후의 각성 걸음(꿈 원료) 복원
        let jpath = dir.join("episodes.journal");
        if jpath.exists() {
            let mut buf = Vec::new();
            use std::io::Read as _;
            std::fs::File::open(&jpath)?.read_to_end(&mut buf)?;
            a.replay_episodes(&buf);
        }
        a.ep_journal = Some(std::io::BufWriter::new(
            std::fs::OpenOptions::new().create(true).append(true).open(&jpath)?,
        ));
        a.ep_dir = Some(dir);
        Ok(a)
    }

    // ---- 에피소드 저널: 레코드 = [0x01][percept][action][state][val] | [0x02][map] ----

    fn ep_record_step(&mut self, s: &EpStep) {
        let Some(j) = self.ep_journal.as_mut() else { return };
        use std::io::Write as _;
        let mut rec = [0u8; 1 + 4 + 2 + 4];
        rec[0] = 0x01;
        rec[1..5].copy_from_slice(&s.percept.to_le_bytes());
        rec[5..7].copy_from_slice(&s.action.to_le_bytes());
        rec[7..11].copy_from_slice(&s.state.to_le_bytes());
        let _ = j.write_all(&rec);
        match &s.val {
            Some(v) => {
                let _ = j.write_all(&[v.used]);
                for i in 0..v.used as usize {
                    let _ = j.write_all(&v.v[i].to_le_bytes());
                }
            }
            None => {
                let _ = j.write_all(&[0xff]);
            }
        }
        self.ep_dirty += 1;
        if self.ep_dirty >= 256 {
            let _ = j.flush();
            self.ep_dirty = 0;
        }
    }

    fn ep_record_boundary(&mut self, map: u32) {
        let Some(j) = self.ep_journal.as_mut() else { return };
        use std::io::Write as _;
        let mut rec = [0u8; 5];
        rec[0] = 0x02;
        rec[1..5].copy_from_slice(&map.to_le_bytes());
        let _ = j.write_all(&rec);
    }

    fn replay_episodes(&mut self, buf: &[u8]) {
        let mut i = 0usize;
        while i < buf.len() {
            match buf[i] {
                0x01 if i + 12 <= buf.len() => {
                    let percept = u32::from_le_bytes(buf[i + 1..i + 5].try_into().unwrap());
                    let action = u16::from_le_bytes(buf[i + 5..i + 7].try_into().unwrap());
                    let state = u32::from_le_bytes(buf[i + 7..i + 11].try_into().unwrap());
                    let used = buf[i + 11];
                    i += 12;
                    let val = if used == 0xff {
                        None
                    } else {
                        let n = (used as usize).min(crate::atom::VAL_DIM);
                        if i + n * 4 > buf.len() {
                            break; // 꼬리 잘림(kill 순간) — 그 걸음만 버린다
                        }
                        let mut v = crate::atom::Val::default();
                        for k in 0..n {
                            v.v[k] =
                                f32::from_le_bytes(buf[i + k * 4..i + k * 4 + 4].try_into().unwrap());
                        }
                        v.used = n as u8;
                        i += n * 4;
                        Some(v)
                    };
                    if let Some(ep) = self.episodes.last_mut() {
                        ep.push(EpStep { percept, action, val, state });
                    }
                }
                0x02 if i + 5 <= buf.len() => {
                    let map = u32::from_le_bytes(buf[i + 1..i + 5].try_into().unwrap());
                    if self.episodes.last().map(|e| !e.is_empty()).unwrap_or(false) {
                        self.episodes.push(Vec::new());
                        self.episode_maps.push(map);
                    }
                    i += 5;
                }
                _ => break, // 알 수 없는 태그/꼬리 잘림 — 안전 중단
            }
        }
    }

    // ---- 지도 상태 스냅숏: 밤의 끝(은퇴 직후)마다 원자적으로 교체 ----

    pub(crate) fn ep_checkpoint(&mut self) {
        let Some(dir) = self.ep_dir.clone() else { return };
        use std::io::Write as _;
        let write_snap = |a: &Agent, path: &std::path::Path| -> std::io::Result<()> {
            let mut w = std::io::BufWriter::new(std::fs::File::create(path)?);
            w.write_all(b"MNDA1\0")?;
            w.write_all(&a.n_maps.to_le_bytes())?;
            w.write_all(&a.active_map.to_le_bytes())?;
            let vecs_u32: [&Vec<u32>; 2] = [&a.node_map, &a.map_cortical];
            for v in vecs_u32 {
                w.write_all(&(v.len() as u32).to_le_bytes())?;
                for &x in v {
                    w.write_all(&x.to_le_bytes())?;
                }
            }
            w.write_all(&(a.map_birth.len() as u32).to_le_bytes())?;
            for &x in &a.map_birth {
                w.write_all(&x.to_le_bytes())?;
            }
            w.write_all(&(a.map_post.len() as u32).to_le_bytes())?;
            for &x in &a.map_post {
                w.write_all(&x.to_le_bytes())?;
            }
            w.write_all(&(a.fresh_clones.len() as u32).to_le_bytes())?;
            for &x in &a.fresh_clones {
                w.write_all(&x.to_le_bytes())?;
            }
            w.flush()
        };
        let tmp = dir.join("agent.snap.tmp");
        if write_snap(self, &tmp).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join("agent.snap"));
        }
        // 소화된 경험은 구조가 됐다 — 저널을 비운다
        self.ep_journal = None;
        let jpath = dir.join("episodes.journal");
        let _ = std::fs::File::create(&jpath);
        self.ep_journal = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&jpath)
            .ok()
            .map(std::io::BufWriter::new);
        self.ep_dirty = 0;
        let _ = self.graph.flush();
    }

    fn load_agent_snap(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Read as _;
        let mut buf = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut buf)?;
        let bad = || std::io::Error::new(std::io::ErrorKind::InvalidData, "agent.snap");
        if buf.len() < 14 || &buf[0..6] != b"MNDA1\0" {
            return Err(bad());
        }
        let mut i = 6usize;
        let mut rd_u32 = |i: &mut usize| -> std::io::Result<u32> {
            if *i + 4 > buf.len() {
                return Err(bad());
            }
            let x = u32::from_le_bytes(buf[*i..*i + 4].try_into().unwrap());
            *i += 4;
            Ok(x)
        };
        self.n_maps = rd_u32(&mut i)?;
        self.active_map = rd_u32(&mut i)?;
        let mut vecs: Vec<Vec<u32>> = Vec::new();
        for _ in 0..2 {
            let n = rd_u32(&mut i)? as usize;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(rd_u32(&mut i)?);
            }
            vecs.push(v);
        }
        self.map_cortical = vecs.pop().unwrap();
        self.node_map = vecs.pop().unwrap();
        let n = rd_u32(&mut i)? as usize;
        let mut birth = Vec::with_capacity(n);
        for _ in 0..n {
            let lo = rd_u32(&mut i)? as u64;
            let hi = rd_u32(&mut i)? as u64;
            birth.push(lo | (hi << 32));
        }
        self.map_birth = birth;
        let n = rd_u32(&mut i)? as usize;
        let mut post = Vec::with_capacity(n);
        for _ in 0..n {
            let x = rd_u32(&mut i)?;
            post.push(f32::from_le_bytes(x.to_le_bytes()));
        }
        self.map_post = post;
        let n = rd_u32(&mut i)? as usize;
        let mut fresh = Vec::with_capacity(n);
        for _ in 0..n {
            fresh.push(rd_u32(&mut i)?);
        }
        self.fresh_clones = fresh;
        // 파생 상태 정합
        if self.node_map.len() < self.graph.n_nodes() {
            self.node_map.resize(self.graph.n_nodes(), 0);
        }
        self.map_chain = vec![Vec::new(); self.n_maps as usize];
        self.episode_maps = vec![self.active_map];
        Ok(())
    }

    /// ?먰뵾?뚮뱶 寃쎄퀎: ?곹깭 異붿쟻???딅뒗??洹몃옒?꾨뒗 ?좎? ??湲곗뼲? ?댁뼱吏꾨떎).
    pub fn reset_episode(&mut self) {
        self.state = None;
        self.belief.clear();
        self.hist.clear();
        self.recent.clear();
        // ?먰뵾?뚮뱶 寃쎄퀎???쇰?? 蹂꾩묶??利앷굅媛 ?꾨땲????移댁슫?곕? ?섍린硫?        // ?쒖옉 吏?먮쭏??怨좎븘 ?대줎???먮씪 吏?꾨? ?ㅼ뿼?쒗궓??
        self.fail_count.clear();
        self.encoder.reset();
        if self.episodes.last().map(|e| !e.is_empty()).unwrap_or(false) {
            self.episodes.push(Vec::new());
            self.episode_maps.push(self.active_map);
            self.ep_record_boundary(self.active_map);
        }
    }

    /// ?쒖꽦 吏?꾩뿉 ?랁븳 ?대줎??
    fn map_clones(&self, percept: u32) -> Vec<u32> {
        self.graph
            .clones_of(percept)
            .iter()
            .copied()
            .filter(|&c| self.node_map.get(c as usize) == Some(&self.active_map))
            .collect()
    }

    /// 吏??m??理쒓렐 沅ㅼ쟻???쇰쭏???ㅻ챸?섎뒗媛 ??**吏????誘우쓬 異붿쟻 ?앹〈??*.
    ///
    /// ?꾩씠 ?섎굹?섎굹瑜??곕줈 蹂대㈃ ???쒕떎: 媛숈? ?댄쐶瑜??곕뒗 ?멸퀎?ㅼ? 吏媛??섏?
    /// 媛꾩꽑 而ㅻ쾭由ъ?媛 吏숈뼱???곗뿰 ?쇱튂 60~80%) ?꾨Т 吏?꾨굹 洹몃윺??빐 蹂댁씤??
    /// ?먮퀎?μ? **?ъ뒳 ?쒖빟**?먯꽌 ?섏삩????誘우쓬??吏???덉뿉???ㅼ젣濡?援대젮蹂닿퀬
    /// 紐?嫄몄쓬?대굹 ?댁븘?⑤뒗吏瑜??쇰떎(= 吏?꾨퀎 HMM ?꾪꽣留??곕룄??洹쇱궗).
    fn map_score(&self, m: u32) -> f32 {
        let fresh_set_l: std::collections::HashSet<u32> =
            self.fresh_clones.iter().copied().collect();
        let h = self.trail.len();
        if h < 2 {
            return 0.0;
        }
        // ?먮퀎?μ? ??뿉 ?щ젮 ?덈떎: ??W ?ъ뒳???됰슧??吏?꾩뿉???곗뿰???댁븘?⑥쓣
        // ?뺣쪧? ?ㅽ뀦????1-(5/6)^W. W=16?대㈃ 0.95(臾댁슜吏臾?, W=3?대㈃ 0.42 ??        // 李?吏?꾩쓽 0.9+? 媛덈씪吏꾨떎. 怨꾩륫?쇰줈 ?뺤씤???섏튂??
        const W: usize = 3;
        let clones_in = |p: u32| -> Vec<u32> {
            let mut v: Vec<u32> = self
                .graph
                .clones_of(p)
                .iter()
                .copied()
                .filter(|&c| self.node_map.get(c as usize) == Some(&m))
                .collect();
            // 利앷굅 ???쒖쑝濡??곸쐞 W媛쒕쭔
            v.sort_unstable_by_key(|&c| std::cmp::Reverse(self.graph.node(c).atom.evidence));
            v.truncate(W);
            v
        };
        // trail? 理쒖떊???????ㅻ옒??履쎈???援대┛??        
        let (p0, _) = self.trail[h - 1];
        let mut belief = clones_in(p0);
        let mut ok = 0usize;
        for i in (0..h - 1).rev() {
            let (p_cur, a) = self.trail[i];
            let mut next: Vec<(u32, u32)> = Vec::new(); // (state, edge count)
            for &b in &belief {
                for s in self.graph.succ(b, a) {
                    // ?뺣┰??媛꾩꽑(移댁슫?멤돟2)留???諛⑷툑 ???ㅼ뿼 媛꾩꽑???띿? ?딅뒗??                    
                    if s.count >= 2
                        && !fresh_set_l.contains(&s.to)
                        && self.node_map.get(s.to as usize) == Some(&m)
                        && self.graph.node(s.to).percept == p_cur
                        && !next.iter().any(|x| x.0 == s.to)
                    {
                        next.push((s.to, s.count));
                    }
                }
            }
            if next.is_empty() {
                belief = clones_in(p_cur); // ?ъ떆?????먯닔 ?놁쓬
            } else {
                ok += 1;
                next.sort_unstable_by_key(|x| std::cmp::Reverse(x.1));
                next.truncate(W);
                belief = next.into_iter().map(|x| x.0).collect();
            }
        }
        ok as f32 / (h - 1) as f32
    }

    /// 吏꾨떒???ㅻ씪??: 吏?꾨? ?몃??먯꽌 吏?뺥븳????媛먯?湲곕? 諛곗젣?섍퀬 ?섎㉧吏 怨꾩링
    /// (?ㅼ퐫??異붿쟻쨌?묎퀬쨌?뺣젹)留?遺꾨━ 寃利앺븯???몄닔遺꾪빐 ?ㅽ뿕???대떎.
    pub fn oracle_set_map(&mut self, m: u32) {
        while self.n_maps <= m {
            self.n_maps += 1;
            self.map_birth.push(self.graph.tick);
        }
        // ?ы썑?뺣쪧???먰빂?쇰줈
        self.map_post = vec![0.001; self.n_maps as usize];
        self.map_post[m as usize] = 1.0;
        self.p_new = 0.02;
        if self.active_map != m {
            self.switch_map(m);
        }
    }

    /// ?뚰봽??吏???쇳빀??????媛깆떊 (媛??d ??寃쎌꽦 ?꾪솚 ?泥?.
    ///
    /// 愿痢〓맂 ?꾩씠 (p_prev, a ??p_cur)媛 媛?吏?꾩쓽 ?뺣┰ 媛꾩꽑(移댁슫?멤돟2)?쇰줈
    /// ?ㅻ챸?섎뒗媛瑜??곕룄濡??쇱븘 P(m)??媛깆떊?쒕떎. ?좎깮 吏??吏볥뒗 以????곕룄 ?섑븳
    /// 0.6?쇰줈 蹂댄샇?섍퀬, 誘몄????멸퀎 媛??m*???곸닔 ?곕룄 0.25濡??곸떆 寃쎌웳?쒗궓????    /// ?대뼡 吏?꾨룄 袁몄????ㅻ챸?섏? 紐삵븯硫?m*媛 而ㅼ졇 ??吏?꾨줈 ?ㅼ껜?붾맂??
    fn soft_map_update(&mut self, p_prev: u32, action: u16, p_cur: u32) {
        let now = self.graph.tick;
        let nm = self.n_maps as usize;
        let fresh_set_l: std::collections::HashSet<u32> =
            self.fresh_clones.iter().copied().collect();
        if self.map_post.len() < nm {
            self.map_post.resize(nm, 0.01);
        }

        if self.map_chain.len() < nm {
            self.map_chain.resize(nm, Vec::new());
        }
        let mut sum = 0.0f32;
        for m in 0..nm as u32 {
            // 吏??m ?덉뿉??誘몃땲 誘우쓬 泥댁씤????嫄몄쓬 援대┛??
            let mut next: Vec<(u32, u32)> = Vec::new();
            for &b in &self.map_chain[m as usize] {
                if (b as usize) >= self.graph.n_nodes() {
                    continue; // ?뺤텞?쇰줈 臾댄슚?붾맂 ?붿옱
                }
                for s in self.graph.succ(b, action) {
                    if s.count >= 2
                        && self.node_map.get(s.to as usize) == Some(&m)
                        && self.graph.node(s.to).percept == p_cur
                        && !next.iter().any(|x| x.0 == s.to)
                    {
                        next.push((s.to, s.count));
                    }
                }
            }
            let l = if next.is_empty() {
                // 泥댁씤 ?щ쭩 ???ъ떆??(?곕룄 ??쓬)
                let mut seed: Vec<(u32, u32)> = self
                    .graph
                    .clones_of(p_cur)
                    .iter()
                    .filter(|&&c| {
                        self.node_map.get(c as usize) == Some(&m)
                            && !fresh_set_l.contains(&c)
                    })
                    .map(|&c| (c, self.graph.node(c).atom.evidence))
                    .collect();
                seed.sort_unstable_by_key(|x| std::cmp::Reverse(x.1));
                seed.truncate(3);
                self.map_chain[m as usize] = seed.into_iter().map(|x| x.0).collect();
                0.25f32
            } else {
                next.sort_unstable_by_key(|x| std::cmp::Reverse(x.1));
                next.truncate(3);
                self.map_chain[m as usize] = next.into_iter().map(|x| x.0).collect();
                0.9f32
            };
            // 吏볥뒗 以묒씤 吏?꾨뒗 ?뺣┰ 媛꾩꽑???놁뼱 ?곕룄媛 ??쾶 ?섏삩?????섑븳?쇰줈 蹂댄샇
            let l = if self.map_cortical.get(m as usize).copied().unwrap_or(0) < 20 {
                l.max(0.6)
            } else {
                l
            };
            let v = (self.map_post[m as usize] + 0.002) * l;
            self.map_post[m as usize] = v;
            sum += v;
        }
        let v_new = (self.p_new + 0.002) * 0.25;
        sum += v_new;
        if sum > 0.0 {
            for v in &mut self.map_post {
                *v /= sum;
            }
            self.p_new = v_new / sum;
        }

        // m* ?ㅼ껜?? 誘몄? 媛?ㅼ씠 吏諛곗쟻?대㈃ ???멸퀎??        
        if self.p_new > 0.65 && self.n_maps < 32 {
            let new = self.n_maps;
            self.n_maps += 1;
            self.map_birth.push(now);
            self.map_post.push(self.p_new);
            self.map_chain.push(Vec::new());
            self.map_cortical.push(0);
            self.p_new = 0.02;
            let s: f32 = self.map_post.iter().sum::<f32>() + self.p_new;
            for v in &mut self.map_post {
                *v /= s;
            }
            self.stats.map_switches += 1;
            self.set_active(new);
            return;
        }

        // argmax瑜??쒖꽦 吏?꾨줈 ???꾪솚??由ъ뀑 鍮꾩슜???녿떎(洹멸쾶 ?뚰봽?몄쓽 ?붿젏)
        let best = self
            .map_post
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i as u32)
            .unwrap_or(0);
        if best != self.active_map {
            self.stats.map_switches += 1;
            self.set_active(best);
        }
    }

    /// ?뚰봽???꾪솚: 移댁슫?곕쭔 吏?곌퀬 誘우쓬쨌臾몃㎘? ?좎??쒕떎(?ㅼ쓬 ?깆쓽 ?ㅼ퐫???꾪꽣媛
    /// ?먯뿰?ㅻ읇寃?嫄몃윭?몃떎). ?먰뵾?뚮뱶??寃쎄퀎瑜??섎씪 吏?꾨퀎 轅?遺꾨━瑜?吏?⑤떎.
    fn set_active(&mut self, m: u32) {
        self.active_map = m;
        self.fail_count.clear();
        self.fail_source.clear();
        // 에피소드는 자르지 않는다. 라이브 전환이 진동하면(신생 지도의 우도 하한
        // 0.6 vs 옛 지도의 간헐적 체인 생존 0.9) 에피소드가 수십 조각으로 파편화되고,
        // 짧은 조각은 밤의 재분류가 신뢰할 수 없게 된다(환경6 포획 연쇄의 실측 원인).
        // 경계 판정은 전부 밤의 몫 — 긴 에피소드가 분류 신뢰도의 원천이다.
    }

    /// 吏꾨떒?? (理쒓렐 32??遺덈웾瑜? 媛?吏?꾩쓽 沅ㅼ쟻 ?앹〈??.
    pub fn map_diag(&self) -> (f32, Vec<f32>) {
        let bad = self.recent_quality.iter().take(32).filter(|&&q| !q).count();
        let rate = if self.recent_quality.is_empty() {
            0.0
        } else {
            bad as f32 / self.recent_quality.len().min(32) as f32
        };
        let scores = (0..self.n_maps).map(|m| self.map_score(m)).collect();
        (rate, scores)
    }

    /// 吏???ъ젙?? "?닿? ?대뒓 ?멸퀎???덈뒗媛"瑜?理쒓렐 沅ㅼ쟻?쇰줈 ?먯젙?쒕떎.
    ///
    /// 媛?吏?꾩뿉 ??? 理쒓렐 (吏媛곣넂吏媛? ?됰룞) ?꾩씠媛 洹?吏?꾩쓽 媛꾩꽑?쇰줈 ?ㅻ챸?섎뒗
    /// 鍮꾩쑉???쇰떎. 異⑸텇???뺥빀??吏?꾧? ?덉쑝硫?洹몃━濡??꾪솚?섍퀬, ?놁쑝硫???吏?꾨?
    /// 媛쒖꽕?쒕떎 ??"?ш린??泥섏쓬 蹂대뒗 ?멸퀎??"
    fn map_relocate(&mut self) {
        let h = self.trail.len();
        if h < 8 {
            return;
        }
        // 嫄댁꽕 以묒씤 吏?꾨뒗 蹂댄샇?쒕떎: 媛??쒖뼱??吏??媛꾩꽑 移댁슫?멸? ?꾩쭅 ?뺤쓬)?먯꽌
        // ?ъ젙?꾨? ?먮떒?섎㈃ "??吏?꾨뒗 ?ㅻ챸?μ씠 ?녿떎"???ㅽ뙋?쇰줈 吏?꾧? 臾댄븳 利앹떇?쒕떎.
        let now = self.graph.tick;
        if now.saturating_sub(self.map_birth.get(self.active_map as usize).copied().unwrap_or(0))
            < 3000
        {
            return;
        }
        let mut best = (self.active_map, -1.0f32);
        for m in 0..self.n_maps {
            let score = self.map_score(m);
            if score > best.1 {
                best = (m, score);
            }
        }

        if best.1 >= 0.65 {
            // ??沅ㅼ쟻???ㅻ챸?섎뒗 吏?꾧? ?덈떎 ??洹몃━濡??꾪솚?쒕떎.
            self.low_streak = 0;
            if best.0 != self.active_map {
                self.switch_map(best.0);
            }
        } else {
            // ?대뼡 吏?꾨룄 ?ㅻ챸?섏? 紐삵븳?? ??吏??媛쒖꽕? 鍮꾩떥誘濡?2???곗냽
            // ?뺤씤 ?꾩뿉留????쇱떆???뚯쓬?쇰줈 ?멸퀎瑜??섎━吏 ?딅뒗??
            self.low_streak += 1;
            if self.low_streak >= 2 {
                self.low_streak = 0;
                let new = self.n_maps;
                self.n_maps += 1;
                self.map_birth.push(now);
                self.switch_map(new);
            }
        }
    }

    fn switch_map(&mut self, target: u32) {
        self.active_map = target;
        self.stats.map_switches += 1;
        self.fail_count.clear();
        self.fail_source.clear();
        self.trail.clear();
        self.recent_quality.clear();
        self.reset_episode(); // ?먰뵾?뚮뱶 寃쎄퀎 ?덈떒 ??吏?꾨퀎 轅?遺꾨━
    }

    /// 誘우쓬???뷀듃濡쒗뵾(鍮꾪듃) ??"吏湲??닿? ?대뵒?몄? ?쇰쭏???뺤떊?섎뒗媛".
    pub fn belief_entropy(&self) -> f32 {
        let mut h = 0.0;
        for &(_, lp) in &self.belief {
            let p = lp.exp2();
            if p > 1e-9 {
                h -= p * p.log2();
            }
        }
        h
    }

    /// 臾몃㎘ 踰≫꽣: 吏湲?蹂대뒗 寃?+ 理쒓렐 紐?嫄몄쓬???섎굹???섏씠?쇰깹?곕줈 臾띕뒗??
    ///
    /// ```text
    /// ctx = 吏媛곣궃 ???(?됰룞?쒋굥????吏媛곣궃?뗢굙) ???짼(?됰룞?쒋굥????吏媛곣궃?뗢굚) ????    /// ```
    ///
    /// `?`(移섑솚)媛 ?쒖꽌瑜??덇린怨? `??(諛붿씤??媛 "洹몃븣 臾댁뾿???섍퀬 臾댁뾿??遊ㅻ뒗媛"瑜?    /// ????쑝濡?臾띔퀬, `??(以묒꺽)媛 ?꾨?瑜??섎굹濡?留뚮뱺?? **湲곗쭏?????곗궛留뚯쑝濡?*
    /// "?ш린媛 ?대뵒?멸?"?쇰뒗 臾쇱쓬???듯븷 ?щ즺媛 留뚮뱾?댁쭊??
    ///
    /// ??踰≫꽣???대줎??二쇱냼媛 ?쒕떎. 媛숈? ?곹솴?대㈃ 媛숈? 踰≫꽣媛 ?섏삤誘濡??곗긽
    /// 硫붾え由ш? 洹몃븣 留뚮뱺 ?대줎???섏갼?붾떎 ???대줎??理쒕떎利앷굅 ?섎굹濡?遺뺢눼?섏? ?딅뒗??
    fn context_vec(&self, percept: u32) -> Sbv {
        let mut b = Bundler::new();
        b.add(&self.graph.percept_vec(percept));
        for (i, &(p, a)) in self.hist.iter().take(self.cfg.context_order).enumerate() {
            let term = self
                .graph
                .percept_vec(p)
                .bind(&Sbv::from_seed(0xAC7100 ^ a as u64));
            b.add(&term.permute(i + 1));
        }
        b.finalize()
    }

    pub fn prefer_percept(&mut self, percept: u32, value: f32) {
        self.pref_percept.insert(percept, value);
    }
    pub fn prefer_state(&mut self, state: u32, value: f32) {
        self.pref_state.insert(state, value);
    }
    pub fn clear_preferences(&mut self) {
        self.pref_percept.clear();
        self.pref_state.clear();
    }

    #[inline]
    fn preference(&self, s: u32) -> f32 {
        let p = self.graph.node(s).percept;
        self.pref_state.get(&s).copied().unwrap_or(0.0)
            + self.pref_percept.get(&p).copied().unwrap_or(0.0)
    }

    // ------------------------------------------------------- B2 ?뺤갑 + B3 ?깆옣

    /// ???? 愿痢≪쓣 諛쏆븘 ?곹깭瑜??뺤젙?섍퀬, ?꾩슂?섎㈃ 援ъ“瑜??섎━怨? ?꾩씠瑜?湲곕줉?쒕떎.
    ///
    /// `action`? **吏곸쟾?????됰룞**?대떎(洹?寃곌낵媛 吏湲덉쓽 愿痢?.
    pub fn perceive(&mut self, obs: &Obs, action: u16) -> Settled {
        let (code, val) = self.encoder.encode(obs);
        self.graph.tick += 1;
        self.stats.ticks += 1;

        let known = self.graph.match_percept(&code, self.cfg.percept_tol);
        let novel_percept = known.is_none();
        let percept = match known {
            Some(p) => p,
            None => self.graph.intern_percept(&code, self.cfg.percept_tol),
        };
        if novel_percept {
            self.stats.novel_percepts += 1;
        }

        // 愿痢?遺덉씪移??? ?먰삎怨??쇰쭏???ㅻⅨ媛
        let obs_bits = {
            let d = code.dist(&self.graph.percept_vec(percept)) as f32;
            self.cfg.obs_weight * (d / crate::sbv::NBLOCKS as f32)
        };

        // --- 誘우쓬 ?꾪뙆: 媛?媛?ㅼ쓣 ??嫄몄쓬 諛怨? 愿痢≪쑝濡?嫄몃윭?몃떎 ---
        //
        // ?닿쾬??"?뺤갑 = 異붾줎"???ㅼ젣 ?뺥깭?? 吏곸쟾 誘우쓬??紐⑤뱺 媛?ㅼ뿉 ????꾩씠
        // ?뺣쪧??怨깊븯怨? 愿痢↔낵 留욎? ?딅뒗 寃껋쓣 ?⑥뼱?⑤┛ ???ㅼ떆 ?뺢퇋?뷀븳??
        // 二쇱쓽(B5): ?꾨낫??'誘우쓬 ?덉쓽 ?곹깭?먯꽌 ???됰룞?쇰줈 媛????덈뒗 怨? ??'??吏媛곸쓽
        // ?대줎'?쇰줈 ?쒗븳?섍퀬 鍮?????곸쑝濡??섎┛????議고빀 ??컻??泥?諛⑹뼱??
        let mut cand: HashMap<u32, f32> = HashMap::new();
        let mut considered = 0usize;

        for &(prev, lp) in &self.belief {
            let succ = self.graph.succ(prev, action);
            let total: f32 = succ.iter().map(|s| s.count as f32).sum();
            // 스무딩 기저는 지도 스코프로 — 후보가 지도 스코프인데 분모가 전역이면,
            // 은퇴 없는 복귀 구간에 쌓인 남의 신선 클론 수백 개가 k를 부풀려 전이
            // 확률을 눌러 만성 놀람을 만든다(복귀 열화가 복귀 "순서"를 따르던 원인).
            let k = self
                .graph
                .clones_of(percept)
                .iter()
                .filter(|&&c| self.node_map.get(c as usize) == Some(&self.active_map))
                .count()
                .max(1) as f32;
            let denom = total + self.cfg.alpha * (k + 1.0);
            for s in succ {
                if self.graph.node(s.to).percept != percept {
                    continue;
                }
                // ?쒖꽦 吏??諛뽰쓽 ?곹깭???꾨낫媛 ?꾨땲????吏??媛??꾪솕 李⑤떒
                if self.node_map.get(s.to as usize) != Some(&self.active_map) {
                    continue;
                }
                considered += 1;
                let p = (s.count as f32 + self.cfg.alpha) / denom;
                let score = lp + p.log2();
                let e = cand.entry(s.to).or_insert(f32::NEG_INFINITY);
                if score > *e {
                    *e = score;
                }
            }
        }

        let mut ranked: Vec<(u32, f32)> = cand.into_iter().collect();
        // 총순서: 점수 동률은 노드 id로 — HashMap 순회 순서가 빔 절단에
        // 새면 프로세스마다 다른 믿음이 자란다(고정 시드 비결정성의 원흉).
        ranked.sort_unstable_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal).then_with(|| a.0.cmp(&b.0))
        });
        ranked.truncate(self.cfg.beam_width);

        // 理쒖긽??媛?ㅼ쓽 ?붿뿬 ?먯쑀?먮꼫吏媛 怨?"?쇰쭏???ㅻ챸?섏? 紐삵뻽?붽?"
        let residual_f = ranked
            .first()
            .map(|&(_, lp)| {
                let prior = self.belief.first().map(|b| b.1).unwrap_or(0.0);
                -(lp - prior) + obs_bits
            })
            .unwrap_or(f32::INFINITY);
        // **?ㅻ챸?섏뿀??= 媛?ν븳 媛?ㅼ씠 ?섎굹?쇰룄 ?⑥븯??**
        //
        // ?뺣쪧????? ?ㅻ챸???ㅻ챸?대떎 ??利됱떆 援ъ“瑜??섎━硫??대줎????컻?쒕떎.
        // 洹몃윭??**?덉쭏???섏걶 ?ㅻ챸????吏媛곸뿉??諛섎났?섎㈃**(理쒖꽑 媛?ㅼ쓽 ?꾩씠 ?뺣쪧??        // 怨꾩냽 0.5 誘몃쭔 = ?붿뿬 F媛 愿痢≫빆??鍮쇨퀬??1鍮꾪듃 珥덇낵) 洹멸쾬? ?≪쓬???꾨땲??        // ?⑥? 臾몃㎘???좏샇????媛숈? ?곹깭?쇨퀬 誘용뒗 怨녹뿉??寃곌낵媛 媛덈씪吏怨??덈떎.
        // 洹몃븣???몃궡 移댁슫?곕? ?щ━怨? ?섏튂硫?遺꾪솕?쒗궓?? 怨쇰텇?붾뒗 轅?EM)???섎룎由곕떎.
        let mut explained =
            !ranked.is_empty() && residual_f <= self.cfg.surprise_threshold;
        // ?덉륫 遺덈웾??**異쒕컻 吏媛?*???곷┰?쒕떎(??fail_source 二쇱꽍 李몄“).
        if explained && (residual_f - obs_bits) > 0.9 {
            if let Some(prev) = self.state {
                let sp = self.graph.node(prev).percept;
                *self.fail_source.entry(sp).or_insert(0) += 1;
            }
        }
        // 遺덈웾???꾩쟻??吏媛곸? ?대쾲 諛⑸Ц?먯꽌 媛뺤젣 遺꾪솕?쒕떎 ???꾩갑 ?ㅻ챸???꾨Т由?        // 洹몃윺??빐?? 嫄곌린???섍????덉륫??怨꾩냽 媛덈씪吏꾨떎硫?洹??뺤껜?깆씠 ?由?寃껋씠??
        // (force???꾨옒 ?깆옣 遺꾧린???꾩갑 ?몃궡 寃뚯씠?몃? ?고쉶?쒕떎 ??吏湲?媛덈씪???쒕떎.)
        let mut force_split = false;
        if explained
            && self.fail_source.get(&percept).map(|&c| c > self.cfg.lost_patience).unwrap_or(false)
            && self.graph.clones_of(percept).len() < self.cfg.max_clones
        {
            explained = false;
            force_split = true;
            self.fail_source.remove(&percept);
        }
        let best = ranked.first().copied();

        // --- 援ъ“ ?숈뒿(B3) ---
        //
        // ??媛덈옒??
        //
        // 1) **?덉륫??愿痢≪쓣 ?ㅻ챸?덈떎** ???멸퀎 紐⑤뜽??留욎븯?? 洹??곹깭瑜??대떎.
        //    ?숈뒿??吏꾪뻾?좎닔濡??遺遺꾩쓽 ?깆씠 ??湲몃줈 媛꾨떎(= 吏꾩쭨 ?곹깭 異붾줎).
        //
        // 2) **?ㅻ챸?섏? 紐삵뻽??* ???ш린媛 ?대뵒?몄? ?덉륫?쇰줈??紐⑤Ⅸ?? ?대븣 **臾몃㎘
        //    踰≫꽣**濡??섎Щ?붾떎: "?대윴 ?곹솴???꾩뿉 寃れ? ???덈뒗媛?" ?덉쑝硫?洹몃븣??        //    ?대줎???섏갼怨? ?놁쑝硫????대줎??留뚮뱾????臾몃㎘??二쇱냼濡??깅줉?쒕떎.
        //
        // 臾몃㎘ 二쇱냼媛 ?놁쑝硫??대줎? ?몄젣??理쒕떎利앷굅 ?섎굹濡?遺뺢눼?쒕떎 ??留뚮뱾?대룄
        // ?섏갼??諛⑸쾿???녾린 ?뚮Ц?대떎. ???섏갼湲곌? 蹂꾩묶 ?멸퀎?먯꽌 吏?꾨? ?몄슫??
        //
        // 媛곸꽦? ?대젃寃??됰꼮??媛덈씪?먭린留??쒕떎. 以묐났? ?섎㈃湲?BMR(C1)???⑹튇??
        let mut grew = false;
        let prev_state = self.state;
        let prev_entropy = self.belief_entropy();
        // ?꾩씠 湲곕줉 ?щ?: ?뺤떊 ?녿뒗 ?꾩씠瑜?吏?꾩뿉 ?곸쑝硫?吏?꾧? ?ㅼ뿼?쒕떎.
        let mut record_link = true;

        let state = if explained {
            self.fail_count.remove(&percept);
            // 誘우쓬???뺢퇋?뷀빐 ?좎??쒕떎 ???щ윭 媛?μ꽦???댁븘 ?덈떎
            let top = ranked[0].1;
            let mut sum = 0.0f32;
            for &(_, lp) in &ranked {
                sum += (lp - top).exp2();
            }
            let norm = sum.log2();
            self.belief = ranked.iter().map(|&(s, lp)| (s, lp - top - norm)).collect();
            best.unwrap().0
        } else if self.map_clones(percept).is_empty() {
            // ??吏?꾩뿉??泥섏쓬 蹂대뒗 吏媛????섏떖??寃??놁씠 ?덈줈??寃껋씠??            grew = true;
            self.stats.clones_grown += 1;
            let ctxv = self.context_vec(percept);
            let id = self.new_clone_in_map(percept);
            self.register_context(ctxv, id);
            self.belief = vec![(id, 0.0)];
            id
        } else {
            // ?꾨뒗 吏媛곸씤???대뼡 媛?ㅻ룄 ?ㅻ챸?섏? 紐삵뻽??????媛吏 媛?μ꽦:
            //   (a) ?좉퉸 湲몄쓣 ?껋뿀???먰뵾?뚮뱶 ?쒖옉쨌轅?吏곹썑쨌?쒕Ц ?ㅼ젙李?
            //   (b) ?뺣쭚 ?덈줈??臾몃㎘?대떎(媛숈? 寃껋씠 ?ㅻⅤ寃??됰룞?섎뒗 ???곹솴)
            //
            // ?ㅽ뙣 ??踰덉쑝濡쒕뒗 (a)? (b)瑜?援щ퀎?????녿떎. 洹몃옒??癒쇱? **?ъ젙??*?쒕떎:
            // 誘우쓬????吏媛곸쓽 紐⑤뱺 ?대줎?쇰줈 ?쇱튂怨? ?ㅼ쓬 愿痢〓뱾??媛?ㅻ궡寃??붾떎.
            // (a)?쇰㈃ 吏?꾧? ?녹쑝誘濡??ㅽ뙣媛 硫롪퀬 移댁슫?곌? 吏?뚯쭊?? (b)?쇰㈃ ??吏媛곸쓽
            // ?ㅽ뙣媛 諛⑸Ц留덈떎 ?볦씤????洹몃븣留?援ъ“瑜??섎┛?? ???몃궡媛 ?섎㈃???몄슫
            // 源⑤걮??吏?꾨? 媛곸꽦???쇰??먯꽌 吏?⑤떎.
            let fc = self.fail_count.entry(percept).or_insert(0);
            *fc += 1;
            let over = force_split || *fc > self.cfg.lost_patience;
            // ?대뵒???붾뒗吏 **紐⑤? ?뚮쭔** ?꾩씠瑜??곸? ?딅뒗?? 吏곸쟾 誘우쓬???뺤떊
            // ?곹깭??ㅻ㈃(?뷀듃濡쒗뵾 ??쓬) 異쒕컻吏??遺꾨챸?섎떎 ??泥섏쓬 ?대낫???됰룞??            // 寃곌낵媛 諛붾줈 ??寃쎌슦?대ŉ, ?닿쾬?????곸쑝硫??먯깋???곸썝???쏅룉??
            record_link = prev_entropy <= 0.5;

            if over && self.map_clones(percept).len() < self.cfg.max_clones {
                self.fail_count.remove(&percept);
                let ctxv = self.context_vec(percept);
                // 지도별 색인: 내 세계의 주소록만 뒤진다 — 남의 지도 그림자가
                // 구조적으로 불가능(전 지도 공용 색인의 그림자 납치 종결).
                let am = self.active_map as usize;
                let known = if am < self.ctx.len() {
                    self.ctx[am]
                        .query(&ctxv, 4)
                        .into_iter()
                        .filter(|h| h.dist <= self.cfg.context_tol)
                        .map(|h| self.ctx_node[am][h.id as usize])
                        .find(|&id| {
                            self.graph.node(id).percept == percept
                                && self.node_map.get(id as usize) == Some(&self.active_map)
                        })
                } else {
                    None
                };
                let s = match known {
                    Some(id) => id,
                    None => {
                        grew = true;
                        self.stats.clones_grown += 1;
                        let id = self.new_clone_in_map(percept);
                        self.register_context(ctxv, id);
                        id
                    }
                };
                // ???곸뿭??媛쒖쿃 以묒씪 ?뚮뒗 ?ъ뒳??湲곕줉?댁빞 ?쒕떎 ??洹멸쾬??EM???먮즺??                record_link = true;
                self.belief = vec![(s, 0.0)];
                s
            } else {
                // ?ъ젙?? ??吏媛곸쓽 紐⑤뱺 ?대줎??**利앷굅 횞 理쒓렐??*?쇰줈 ?대젮?붾떎.
                //
                // ?됱깮 利앷굅留??곕㈃ ???멸퀎??怨좎쬆嫄??대줎???곸썝???닿릿???????섍꼍??                // ?ㅼ뼱?쒕룄 ???섍꼍 ?곹깭濡??뚮젮媛怨? ???꾩씠媛 ??援ъ“瑜??ㅼ뿼?쒗궓??                // (10?섍꼍 ?곗냽 ?숈뒿 遺뺢눼???먯씤?쇰줈 ?ㅼ륫 ?뺤씤). "諛⑷툑 ?꾧퉴吏 ?덈뜕
                // ?멸퀎???꾩쭅 ?덉쓣 ?뺣쪧???믩떎"??理쒓렐???ъ쟾遺꾪룷媛 ?곗냽 ?숈뒿??                // ?꾩슂議곌굔?대떎.
                // 理쒓렐??踰뚯젏? 吏??異붾줎 紐⑤뱶 ?꾩슜?대떎: 洹?紐⑤뱶?먯꽌??"諛⑷툑 ?꾩쓽
                // ?멸퀎" 媛以묒씠 ?꾩슂?섏?留? ?⑥씪 吏??EM??臾몃㎘??留〓뒗) 紐⑤뱶?먯꽌 耳쒕㈃
                // ?ㅻ옒???섍꼍?쇰줈 蹂듦?????李??대줎???섏씠 踰뚯젏?쇰줈 ?⑤같?쒕떎(?ㅼ륫).
                let now = self.graph.tick;
                let clones = self.map_clones(percept);
                let mut total = 0.0f32;
                let weights: Vec<(u32, f32)> = clones
                    .iter()
                    .map(|&c| {
                        let a = &self.graph.node(c).atom;
                        let age = if self.cfg.map_inference {
                            (now.saturating_sub(a.t)) as f32 / 1000.0
                        } else {
                            0.0
                        };
                        let w = (a.evidence as f32 + 1.0) / (1.0 + age);
                        total += w;
                        (c, w)
                    })
                    .collect();
                let mut bel: Vec<(u32, f32)> = weights
                    .into_iter()
                    .map(|(c, w)| (c, (w / total).log2()))
                    .collect();
                bel.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
                bel.truncate(self.cfg.beam_width.max(4));
                let s = bel[0].0;
                self.belief = bel;
                s
            }
        };

        let rf = if residual_f.is_finite() { residual_f } else { obs_bits + 8.0 };
        self.stats.surprise_sum += rf as f64;

        // --- 湲곕줉: ?숈뒿???꾨? ---
        // 理쒕퉰 媛??寃쎈줈?먮쭔 ?대떎. 誘우쓬??媛덈씪???덉뼱??援ъ“???섎굹留??먮???
        self.graph.visit(state, val);
        if record_link {
            // 지도 확신이 낮은 순간(전환 진동 창)에는 링크를 기록하지 않는다 —
            // 진동 창의 기록이 옛 지도의 정갈한 피질 위에 외부 전이의 간선 그물을
            // 짜면, 노드 수는 그대로인 채 어떤 시퀀스든 연장되는 "간선 blob"이
            // 된다(측정診 생존율 1.0 조작 — 복귀 오잠금의 원흉, 시드 2029 검거).
            // 에피소드는 온전히 보존되므로 진실의 원천(밤의 EM)은 잃는 것이 없다.
            let confident = !self.cfg.map_inference
                || self
                    .map_post
                    .get(self.active_map as usize)
                    .copied()
                    .unwrap_or(1.0)
                    >= 0.7;
            if confident {
                if let Some(prev) = prev_state {
                    self.graph.link(prev, action, state);
                }
            }
        }
        self.state = Some(state);
        self.last_action = action;
        self.hist.insert(0, (percept, action));
        self.hist.truncate(self.cfg.context_order.max(1));
        if let Some(prev) = prev_state {
            self.recent.insert(0, (prev, action));
            self.recent.truncate(8);
        }
        let step = EpStep { percept, action, val, state };
        if let Some(ep) = self.episodes.last_mut() {
            ep.push(step);
        }
        self.ep_record_step(&step);

        // ---- 吏??異붾줎: ?뚰봽???쇳빀 (媛??d) ----
        // 寃쎌꽦 ?꾪솚(?앹〈???꾧퀎 + 由ъ뀑)? ?쒖쟻 媛??3醫낆쑝濡쒕룄 紐??대┛ 泥닿퀎??        // 寃고븿?쇰줈 ?먯젙?먮떎(?쒕룄 28~32). ?ы썑?뺣쪧 P(m)??留???媛깆떊?섎뒗 ?곗냽
        // 踰꾩쟾?쇰줈 ?泥????ㅽ뙋? ?ㅼ쓬 愿痢〓뱾濡??먭린?섏젙?섍퀬 ?꾪솚 鍮꾩슜???녿떎.
        self.trail.insert(0, (percept, action));
        self.trail.truncate(24);
        let good = explained && (residual_f - obs_bits) <= 0.9;
        self.recent_quality.insert(0, good);
        self.recent_quality.truncate(48);
        if self.map_check_grace > 0 {
            self.map_check_grace -= 1;
        }
        if self.cfg.map_inference && self.map_check_grace == 0 {
            // hist[0]? 諛⑷툑 ?ｌ? ?꾩옱 吏媛????꾩씠??異쒕컻? hist[1]?대떎
            if let Some(&(pp, _)) = self.hist.get(1) {
                self.soft_map_update(pp, action, percept);
            }
        }

        Settled {
            state,
            percept,
            residual_f: rf,
            grew,
            novel_percept,
            considered,
        }
    }

    pub(crate) fn register_context(&mut self, ctxv: Sbv, node: u32) {
        let am = self.active_map as usize;
        while self.ctx.len() <= am {
            self.ctx.push(Store::new());
            self.ctx_node.push(Vec::new());
        }
        let ci = self.ctx_node[am].len() as u32;
        self.ctx[am].insert(ci, ctxv);
        self.ctx_node[am].push(node);
    }

    /// ?섎㈃ ?⑥뒪 ??媛곸꽦???됰꼮??媛덈씪??援ъ“瑜??뺤텞?쒕떎(C1 BMR).
    ///
    /// ?됰룞?쇰줈 援щ퀎?섏? ?딅뒗 ?곹깭?ㅼ쓣 ?⑹튂怨? 利앷굅 ?녿뒗 媛꾩꽑??嫄룹뼱?몃떎.
    /// ?곹깭 踰덊샇媛 諛붾뚮?濡?臾몃㎘ ?됱씤怨??좏샇???④퍡 ??릿??
    pub fn sleep(&mut self, cfg: crate::sleep::SleepConfig) -> crate::sleep::SleepReport {
        let groups = self.node_map.clone();
        let (rep, map) = crate::sleep::consolidate_grouped(
            &mut self.graph,
            cfg,
            if self.n_maps > 1 { Some(&groups) } else { None },
        );
        self.apply_node_map(&map);
        rep
    }

    /// 洹몃옒???몃뱶 踰덊샇媛 ?щ같?대릱????`map[old] = new`, ??젣??MAX) ?먯씠?꾪듃??    /// 遺???곹깭瑜??곕씪 ??릿?? bisim ?섎㈃怨??묎퀬??轅덉씠 怨듭쑀?쒕떎.
    pub(crate) fn apply_node_map(&mut self, map: &[u32]) {
        // 臾몃㎘ ?됱씤 ?댁궗: ?щ씪吏??곹깭瑜?媛由ы궎??臾몃㎘? 踰꾨┛??        
        let n_stores = self.ctx.len();
        let mut ctx: Vec<Store> = (0..n_stores).map(|_| Store::new()).collect();
        let mut ctx_node: Vec<Vec<u32>> = vec![Vec::new(); n_stores];
        for m in 0..n_stores {
            for (ci, &old) in self.ctx_node[m].iter().enumerate() {
                let new = match map.get(old as usize) {
                    Some(&n) => n,
                    None => continue,
                };
                if new == u32::MAX {
                    continue;
                }
                if let Some(v) = self.ctx[m].get(ci as u32) {
                    ctx[m].insert(ctx_node[m].len() as u32, *v);
                    ctx_node[m].push(new);
                }
            }
        }
        self.ctx = ctx;
        self.ctx_node = ctx_node;

        // ?대쭏 ?붿쟻 紐⑸줉????踰덊샇濡??댁븘?⑥? 寃껊쭔)
        let mut fresh = Vec::with_capacity(self.fresh_clones.len());
        for &old in &self.fresh_clones {
            if let Some(&n) = map.get(old as usize) {
                if n != u32::MAX {
                    fresh.push(n);
                }
            }
        }
        self.fresh_clones = fresh;

        // ?몃뱶?믪???諛곗뿴????踰덊샇濡?        
        let n_new = self.graph.n_nodes();
        let mut nm = vec![0u32; n_new];
        for (old, &new) in map.iter().enumerate() {
            if new != u32::MAX {
                if let Some(&m) = self.node_map.get(old) {
                    nm[new as usize] = m;
                }
            }
        }
        self.node_map = nm;

        // ?좏샇? ?꾩옱 ?곹깭????踰덊샇濡?        
        let mut pref = HashMap::new();
        for (&s, &v) in &self.pref_state {
            if let Some(&n) = map.get(s as usize) {
                if n != u32::MAX {
                    *pref.entry(n).or_insert(0.0f32) += v;
                }
            }
        }
        self.pref_state = pref;
        self.state = self.state.and_then(|s| match map.get(s as usize) {
            Some(&n) if n != u32::MAX => Some(n),
            _ => None,
        });
        self.belief.clear();
        if let Some(s) = self.state {
            self.belief.push((s, 0.0));
        }
        self.recent.clear();
        // ?몃뱶 踰덊샇媛 諛붾뚯뿀?쇰땲 吏??泥댁씤? ?ъ떆?쒓? ?꾩슂?섎떎
        for c in &mut self.map_chain {
            c.clear();
        }
        // 피질 카운트 재계산
        {
            let fresh: std::collections::HashSet<u32> = self.fresh_clones.iter().copied().collect();
            let mut mc = vec![0u32; self.n_maps as usize];
            for (i, &m) in self.node_map.iter().enumerate() {
                if (m as usize) < mc.len() && !fresh.contains(&(i as u32)) {
                    mc[m as usize] += 1;
                }
            }
            self.map_cortical = mc;
        }
    }

    /// 洹몃옒?꾧? ?듭㎏濡??ш굔????轅? ?먯씠?꾪듃??遺???곹깭瑜???吏?꾩뿉 留욎텣??
    pub(crate) fn after_remap(&mut self) {
        self.ctx = vec![Store::new()];
        self.ctx_node = vec![Vec::new()];
        self.state = None;
        self.belief.clear();
        self.hist.clear();
        self.recent.clear();
        self.trail.clear();
        self.recent_quality.clear();
        self.fail_count.clear();
        self.fail_source.clear();
        self.fresh_clones.clear();
        // ?꾨㈃ ?ш굔? ?⑥씪 吏??紐⑤뱶??        self.n_maps = 1;
        self.active_map = 0;
        self.node_map = vec![0; self.graph.n_nodes()];
        self.episode_maps = vec![0];
        self.map_birth = vec![0];
        self.map_post = vec![1.0];
        self.p_new = 0.02;
        self.map_chain = vec![Vec::new()];
        self.since_map_check = 0;
        self.pref_state.clear(); // ?곹깭 踰덊샇媛 臾댄슚 ??吏媛??좏샇???좎??쒕떎
    }

    /// ?덈줈 留뚮뱾吏 ?딄퀬 湲곗〈 ?대줎 ?섎굹瑜?怨좊Ⅸ???덉궛 ?뚯쭊 ?쒖쓽 理쒗썑 ?섎떒).
    /// ?ъ젙?꾩? 媛숈? 利앷굅횞理쒓렐??湲곗?, ?쒖꽦 吏???곗꽑.
    fn reuse_clone(&self, percept: u32) -> u32 {
        let now = self.graph.tick;
        let pool = self.map_clones(percept);
        let pool = if pool.is_empty() {
            self.graph.clones_of(percept).to_vec()
        } else {
            pool
        };
        let use_recency = self.cfg.map_inference;
        pool.into_iter()
            .max_by(|&a, &b| {
                let s = |c: u32| {
                    let at = &self.graph.node(c).atom;
                    let age = if use_recency {
                        (now.saturating_sub(at.t)) as f32 / 1000.0
                    } else {
                        0.0
                    };
                    (at.evidence as f32 + 1.0) / (1.0 + age)
                };
                s(a).partial_cmp(&s(b)).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap()
    }

    /// ?쒖꽦 吏?꾩뿉 ???대줎??留뚮뱺??紐⑤뱺 媛곸꽦 ?대줎 ?앹꽦???⑥씪 愿臾?.
    fn new_clone_in_map(&mut self, percept: u32) -> u32 {
        let id = self.graph.new_clone(percept);
        let i = id as usize;
        if self.node_map.len() <= i {
            self.node_map.resize(i + 1, 0);
        }
        self.node_map[i] = self.active_map;
        self.fresh_clones.push(id);
        id
    }

    // ---------------------------------------------------------- B4 怨꾪쉷 + B5 ?덉궛

    /// 湲곕??먯쑀?먮꼫吏 G瑜?理쒖냼?뷀븯??泥??됰룞??怨좊Ⅸ??
    ///
    /// 洹몃옒????理쒖쟻 ?곗꽑 ?먯깋(anytime). ?덉궛?????곕㈃ 洹??쒖젏源뚯???理쒖꽑??    /// ?뚮젮以?????쒓컙???놁쑝硫?諛섏궗, ?덉쑝硫??숆퀬.
    ///
    /// G???④퀎 鍮꾩슜:
    /// ```text
    /// cost = 1                        吏㏃? 寃쎈줈 ?좏샇
    ///      + 貫_unc 쨌 (?뭠og?괦(s'|s,a))  誘우쓣 留뚰븳 ?꾩씠 ?좏샇
    ///      ??貫_info 쨌 H(s,a)           遺덊솗?ㅽ븳 怨??좏샇 (= ?멸린??
    ///      ??pref(s')                  ?좏샇 ?곹깭 ?좏샇
    /// ```
    /// ?뺣낫 ?대뱷 ??쓽 遺?멸? ?뚯닔?쇰뒗 ?먯씠 ?듭떖?대떎: **紐⑤Ⅴ??怨녹쑝濡?媛??寃껋씠
    /// ?몃떎.** ?먯깋??蹂꾨룄 ?뺤콉???꾨땲??紐⑹쟻?⑥닔?먯꽌 ?섏삩??
    ///
    /// ?? ?좏샇媛 ?ㅼ젙???덉쑝硫?紐⑺몴 異붽뎄) ?뺣낫 ?대뱷 ??쓣 ?덈떎 ??EFE???뺣???    /// 媛以묎낵 媛숈? ?댁튂濡? ?먰븯??寃껋씠 遺꾨챸???뚮뒗 ?ㅼ슜 媛移섍? 吏諛고빐???쒕떎.
    /// 耳??먮㈃ "遺덊솗?ㅽ븳 媛꾩꽑"??媛믪떥寃?蹂댁뿬 ?먯깋???섏긽 寃쎈줈媛 泥??됰룞??留???    /// ?ㅼ쭛?붾떎(吏꾨떒?먯꽌 ?뺤씤????移??뺣났???먯씤).
    pub fn plan(&mut self, n_actions: u16) -> Option<u16> {
        self.state?;
        let goal_directed = !self.pref_state.is_empty() || !self.pref_percept.is_empty();
        let info_w = if goal_directed { 0.0 } else { self.cfg.info_weight };
        let mut heap: BinaryHeap<Cand> = BinaryHeap::new();
        let mut seen: HashMap<u32, f32> = HashMap::new();
        let mut best_any: Option<(f32, u16)> = None;
        let mut expansions = 0usize;

        // 誘우쓬 遺꾪룷 ?꾩껜?먯꽌 異쒕컻?쒕떎(?ㅼ쨷 ?뚯뒪). 理쒕퉰 媛???섎굹留?誘욧퀬 異쒕컻?섎㈃
        // 洹?媛?ㅼ씠 怨좎븘 ?곹깭(媛꾩꽑 ?놁쓬)????怨꾪쉷 ?꾩껜媛 二쎈뒗??????踰덉㎏ 媛?ㅼ씠
        // ?녹쓣 ?섎룄 ?덈떎??寃껋쓣 怨꾪쉷???뚯븘???쒕떎. 媛??뚯뒪??珥덇린 鍮꾩슜?
        // 洹?媛?ㅼ쓽 遺덊솗?ㅼ꽦(-log P)?대떎.
        for &(s, lp) in &self.belief {
            let c0 = -lp * 0.5;
            heap.push(Cand { cost: c0, node: s, first: u16::MAX, depth: 0 });
            seen.insert(s, c0);
        }

        while let Some(cur) = heap.pop() {
            if expansions >= self.cfg.plan_budget || usize::from(cur.depth) >= self.cfg.plan_depth {
                if cur.first != u16::MAX {
                    let score = cur.cost - self.preference(cur.node) * 4.0;
                    if best_any.is_none() || score < best_any.unwrap().0 {
                        best_any = Some((score, cur.first));
                    }
                }
                if expansions >= self.cfg.plan_budget {
                    break;
                }
                continue;
            }
            expansions += 1;

            // ?좏샇 ?곹깭???우븯?쇰㈃ 利됱떆 梨꾪깮(理쒖쟻 ?곗꽑?대?濡?理쒖쟻 寃쎈줈)
            if cur.first != u16::MAX && self.preference(cur.node) > 0.0 {
                self.stats.plan_expansions += expansions as u64;
                return Some(cur.first);
            }

            for a in 0..n_actions {
                let succ = self.graph.succ(cur.node, a);
                let h = self.graph.entropy(cur.node, a);

                if succ.is_empty() {
                    // ??踰덈룄 ???대낯 ?됰룞 ??紐⑺몴媛 ?놁쓣 ?뚮뒗 理쒓퀬???꾨낫(?뺣낫 ?대뱷 理쒕?),
                    // 紐⑺몴 異붽뎄 以묒뿉???꾨뒗 湲몄씠 ?꾪? ?놁쓣 ?뚯쓽 理쒗썑 ?섎떒.
                    let bonus = if goal_directed { -2.0 } else { self.cfg.info_weight };
                    let cost = cur.cost + 1.0 - bonus;
                    let first = if cur.first == u16::MAX { a } else { cur.first };
                    if best_any.is_none() || cost < best_any.unwrap().0 {
                        best_any = Some((cost, first));
                    }
                    continue;
                }

                // 諛⑷툑 諛잛? 湲몄? ?좎떆 鍮꾩떥????理쒓렐?쇱닔濡?臾닿쾪寃??쒖옄由ш구???덉텧)
                let tabu: f32 = self
                    .recent
                    .iter()
                    .enumerate()
                    .filter(|(_, &(s, ac))| s == cur.node && ac == a)
                    .map(|(i, _)| 3.0 / (i as f32 + 1.0))
                    .sum();

                let total: f32 = succ.iter().map(|s| s.count as f32).sum();
                // 전환 비용: 뿌리 확장(첫 행동 결정)에서만, 직전 행동과 다르면 부과
                let sw = if cur.first == u16::MAX && a != self.last_action {
                    self.cfg.switch_cost
                } else {
                    0.0
                };
                for s in succ {
                    let p = s.count as f32 / total;
                    let step = 1.0 + tabu + sw + self.cfg.uncertainty_weight * (-p.log2())
                        - info_w * h
                        - self.preference(s.to);
                    let cost = cur.cost + step.max(0.05);
                    if let Some(&old) = seen.get(&s.to) {
                        if old <= cost {
                            continue;
                        }
                    }
                    seen.insert(s.to, cost);
                    let first = if cur.first == u16::MAX { a } else { cur.first };
                    heap.push(Cand { cost, node: s.to, first, depth: cur.depth + 1 });
                    let score = cost - self.preference(s.to) * 4.0;
                    if best_any.is_none() || score < best_any.unwrap().0 {
                        best_any = Some((score, first));
                    }
                }
            }
        }

        self.stats.plan_expansions += expansions as u64;
        best_any.map(|b| b.1)
    }

    /// ?덉궛????텣 諛섏궗???됰룞 ?좏깮(System 1). 媛숈? 猷⑦봽, ?곸? 諛섎났.
    pub fn react(&mut self, n_actions: u16) -> Option<u16> {
        let saved = self.cfg.plan_budget;
        self.cfg.plan_budget = 24;
        let a = self.plan(n_actions);
        self.cfg.plan_budget = saved;
        a
    }

    /// ?꾩쭅 ?쒕룄?섏? ?딆? ?됰룞???곗꽑?섎뒗 ?먯깋 ?됰룞(遺??珥덇린??.
    pub fn explore(&mut self, n_actions: u16, rng: &mut Rng) -> u16 {
        if let Some(s) = self.state {
            let tried = self.graph.actions_from(s);
            let untried: Vec<u16> = (0..n_actions).filter(|a| !tried.contains(a)).collect();
            if !untried.is_empty() {
                return untried[rng.below(untried.len() as u32) as usize];
            }
        }
        rng.below(n_actions as u32) as u16
    }

    /// ?됯퇏 ??쇱?(鍮꾪듃) ???멸퀎瑜??쇰쭏?????ㅻ챸?섍퀬 ?덈뒗吏???⑥씪 吏??
    pub fn mean_surprise(&self) -> f64 {
        if self.stats.ticks == 0 {
            0.0
        } else {
            self.stats.surprise_sum / self.stats.ticks as f64
        }
    }

    /// ?꾩옱 ?곹깭?먯꽌 ?덉륫???ㅼ쓬 吏媛?寃??媛?μ꽦).
    pub fn predict_percept(&self, action: u16) -> Option<(u32, f32)> {
        let s = self.state?;
        let (next, p) = self.graph.predict(s, action)?;
        Some((self.graph.node(next).percept, p))
    }

    /// ?꾩옱 ?곹깭?먯꽌 ?덉륫???ㅼ쓬 ?곗냽??Bounce Test媛 ?곕뒗 ?뺥깭).
    pub fn predict_value(&self, action: u16) -> Option<Val> {
        let s = self.state?;
        let (next, _) = self.graph.predict(s, action)?;
        self.graph.node(next).atom.value
    }
}

impl Default for Agent {
    fn default() -> Self {
        Agent::new()
    }
}

// 理쒖냼 ?숈쓣 ?꾪븳 ??닚 鍮꾧탳
#[derive(Clone, Copy, PartialEq)]
struct Cand {
    cost: f32,
    node: u32,
    first: u16,
    depth: u16,
}
impl Eq for Cand {}
impl Ord for Cand {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .partial_cmp(&self.cost)
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.node.cmp(&other.node))
    }
}
impl PartialOrd for Cand {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::Obs;

    /// M1 마감: 에피소드 저널 + 지도 상태 스냅숏의 kill -9 복구.
    /// 각성 도중 프로세스가 죽어도(드롭) 꿈 원료(에피소드)와 지도 소속이 남아야 한다.
    #[test]
    fn episode_journal_survives_kill() {
        let dir = std::env::temp_dir().join(format!("monad_epj_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let role = 0u16;
        let steps_before;
        {
            let mut a = Agent::attach(&dir, Config::default()).unwrap();
            a.encoder.declare(role, "cell");
            for step in 0..300 {
                let cell = (step % 5) as u32;
                a.perceive(&Obs::new().cat(role, cell), (step % 2) as u16);
            }
            a.reset_episode(); // 경계 기록
            for step in 0..200 {
                let cell = (step % 5) as u32;
                a.perceive(&Obs::new().cat(role, cell), (step % 3) as u16);
            }
            steps_before = a.episodes.iter().map(|e| e.len()).sum::<usize>();
            // kill -9 시뮬레이션: flush 없이… 는 아니고 버퍼는 밀어둔다(256걸음마다
            // 자동 flush되므로 실제 손실은 최대 255걸음 — 여기선 명시 flush로 검증).
            if let Some(j) = a.ep_journal.as_mut() {
                use std::io::Write as _;
                let _ = j.flush();
            }
            let _ = a.graph.flush();
            // 드롭 = 프로세스 종료와 동형(저널은 append-only)
        }
        {
            let a = Agent::attach(&dir, Config::default()).unwrap();
            let steps_after: usize = a.episodes.iter().map(|e| e.len()).sum();
            assert!(
                steps_after + 4 >= steps_before,
                "복구된 걸음 {steps_after} < 기록 {steps_before}"
            );
            assert!(a.episodes.len() >= 2, "에피소드 경계 소실: {}", a.episodes.len());
            assert_eq!(a.node_map.len(), a.graph.n_nodes(), "node_map 길이 불일치");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 밤의 끝 체크포인트: 저널이 비워지고 지도 상태 스냅숏이 남아 재접속에 반영된다.
    #[test]
    fn night_checkpoint_clears_journal_and_saves_maps() {
        let dir = std::env::temp_dir().join(format!("monad_epc_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let role = 0u16;
        {
            let mut a = Agent::attach(&dir, Config::default()).unwrap();
            a.encoder.declare(role, "cell");
            for step in 0..600 {
                let cell = (step % 6) as u32;
                a.perceive(&Obs::new().cat(role, cell), (step % 4) as u16);
            }
            a.n_maps = 3; // 지도 상태가 스냅숏에 실리는지 표식
            a.map_birth = vec![0, 1, 2];
            a.map_post = vec![0.5, 0.3, 0.2];
            a.map_cortical = vec![0, 0, 0];
            let rep = crate::dream::dream(
                &mut a,
                crate::dream::DreamConfig { consume: true, max_clones: 8, ..Default::default() },
            );
            assert!(rep.nodes_after > 0);
            let jlen = std::fs::metadata(dir.join("episodes.journal")).unwrap().len();
            assert!(jlen < 64, "밤 후 저널이 안 비워짐: {jlen}바이트");
            assert!(dir.join("agent.snap").exists(), "agent.snap 미생성");
        }
        {
            let a = Agent::attach(&dir, Config::default()).unwrap();
            assert_eq!(a.n_maps, 3, "지도 수 복구 실패");
            assert_eq!(a.map_birth, vec![0, 1, 2]);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 寃곗젙濡좎쟻 ?ъ뒳 ?멸퀎: ?곹깭 0????????, 愿痢≪? ?꾨? ?ㅻ쫫.
    #[test]
    fn learns_deterministic_chain() {
        let mut a = Agent::new();
        let role = 0u16;
        a.encoder.declare(role, "cell");
        let n = 4u32;
        for step in 0..40 {
            let cell = (step % n) as u32;
            a.perceive(&Obs::new().cat(role, cell), 0);
        }
        assert_eq!(a.graph.n_percepts(), 4);
        assert!(a.graph.n_nodes() <= 8, "?대줎 ??컻: ?몃뱶 {}", a.graph.n_nodes());
        let mut s = 0.0;
        for step in 40..50 {
            let cell = (step % n) as u32;
            s += a.perceive(&Obs::new().cat(role, cell), 0).residual_f;
        }
        assert!(s / 10.0 < 1.0, "?숈뒿 ???됯퇏 ??쇱? {}", s / 10.0);
    }

    /// 蹂꾩묶 ?멸퀎: ???곹깭媛 媛숈? 愿痢≪쓣 ?몃떎. ?대줎??媛덈씪吏怨? 轅덉씠 吏?꾨? ?꾩꽦?쒕떎.
    ///
    /// 媛곸꽦留뚯쑝濡쒕뒗 洹쇱궗 吏???ъ젙??以?湲곕줉?섏? ?딆? 媛꾩꽑??怨듬갚)媛 ?⑤뒗 寃껋씠
    /// ?뺤긽?대떎 ???꾩꽦? ?섎㈃(轅?EM)??紐レ씠?쇰뒗 遺꾩뾽?????쒖뒪?쒖쓽 ?ㅺ퀎??
    #[test]
    fn splits_clones_under_aliasing() {
        // ?쒗솚: A B A C  (B? C???ㅻⅤ吏留???A??媛숈? 愿痢? ?ㅻⅨ 臾몃㎘)
        let mut a = Agent::new();
        let role = 0u16;
        a.encoder.declare(role, "cell");
        let seq = [0u32, 1, 0, 2];
        for step in 0..80 {
            a.perceive(&Obs::new().cat(role, seq[step % 4]), 0);
        }
        assert_eq!(a.graph.n_percepts(), 3, "m");
        let clones_a = a.graph.clones_of(0).len();
        assert!(clones_a >= 2, "m");

        // 轅? ?꾩뿭 異붾줎?쇰줈 吏?꾨? ?ㅼ떆 ?몄슫??        
        crate::dream::dream(&mut a, crate::dream::DreamConfig::default());
        a.reset_episode();

        // ?꾩꽦??吏?꾨씪硫??쒗솚 ?꾩껜媛 ?ㅻ챸?섏뼱???쒕떎(?뺤갑 ?뚮컢??紐????쒖쇅)
        let mut correct = 0;
        for step in 0..44 {
            let s = a.perceive(&Obs::new().cat(role, seq[step % 4]), 0);
            if step >= 4 && s.residual_f < 1.0 {
                correct += 1;
            }
        }
        assert!(correct >= 38, "轅??댄썑 ?ㅻ챸 ?깃났 {correct}/40");
        // 洹몃━怨?A????臾몃㎘?쇰줈 議댁옱?댁빞 ?쒕떎
        assert!(a.graph.clones_of(0).len() >= 2, "轅??댄썑 A ?대줎 ?뚯떎");
    }

    #[test]
    fn planning_reaches_preferred_state() {
        let mut a = Agent::new();
        let role = 0u16;
        a.encoder.declare(role, "cell");
        for step in 0..60 {
            a.perceive(&Obs::new().cat(role, (step % 4) as u32), 0);
        }
        a.reset_episode();
        a.perceive(&Obs::new().cat(role, 0), 0);
        a.prefer_percept(3, 5.0);
        assert_eq!(a.plan(2), Some(0), "?좏샇 ?곹깭濡?媛???됰룞??怨⑤씪???쒕떎");
    }

    #[test]
    fn explore_prefers_untried_actions() {
        let mut a = Agent::new();
        let role = 0u16;
        a.encoder.declare(role, "cell");
        a.perceive(&Obs::new().cat(role, 0), 0);
        let mut r = Rng::new(1);
        a.perceive(&Obs::new().cat(role, 1), 0);
        a.reset_episode();
        a.perceive(&Obs::new().cat(role, 0), 0);
        for _ in 0..20 {
            assert_ne!(a.explore(4, &mut r), 0, "?대? ?대낯 ?됰룞???ㅼ떆 怨좊Ⅴ硫????쒕떎");
        }
    }

    #[test]
    fn novel_percept_is_flagged() {
        let mut a = Agent::new();
        let role = 0u16;
        a.encoder.declare(role, "cell");
        assert!(a.perceive(&Obs::new().cat(role, 0), 0).novel_percept);
        a.reset_episode();
        assert!(!a.perceive(&Obs::new().cat(role, 0), 0).novel_percept);
    }

    /// 臾몃㎘ 踰≫꽣媛 ?쒕줈 ?ㅻⅨ ?곹솴???ㅼ젣濡?援щ퀎?섎뒗媛.
    #[test]
    fn context_vector_discriminates() {
        let mut a = Agent::new();
        let role = 0u16;
        a.encoder.declare(role, "cell");
        // 媛숈? 吏媛?0???쒕줈 ?ㅻⅨ ?대젰?먯꽌 留뚮굹寃??쒕떎
        a.perceive(&Obs::new().cat(role, 1), 0);
        a.perceive(&Obs::new().cat(role, 0), 0);
        let c1 = a.context_vec(0);
        a.reset_episode();
        a.perceive(&Obs::new().cat(role, 2), 0);
        a.perceive(&Obs::new().cat(role, 0), 0);
        let c2 = a.context_vec(0);
        assert!(c1.sim(&c2) < 0.8, "?ㅻⅨ 臾몃㎘??援щ퀎?쇱빞: sim={}", c1.sim(&c2));
        // 媛숈? ?대젰??諛섎났?섎㈃ 媛숈? 踰≫꽣
        a.reset_episode();
        a.perceive(&Obs::new().cat(role, 2), 0);
        a.perceive(&Obs::new().cat(role, 0), 0);
        assert_eq!(a.context_vec(0), c2, "媛숈? 臾몃㎘? 媛숈? 踰≫꽣?ъ빞");
    }
}
