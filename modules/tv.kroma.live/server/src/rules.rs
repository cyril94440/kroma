use kroma_module_sdk::host::HostCtx;

pub const DEFAULT_BLOCKED: &str = "adult,xxx,porn,+18,18+";
pub const DEFAULT_FOREIGN: &str = "be,uk,gb,us,ca,de,at,ch,lu,nl,it,es,pt,pl,ar,tr,gr,se,no,dk,fi,cz,sk,hu,ro,bg,ru,ua,br,mx,in,qa,sa,ae,ma,dz,tn,eg,il,ir,hr,rs,si,ba,mk,al,lt,lv,ee,af,now,prime,sr,srb,ex,yu,exyu,cro,bih,mne,za,pk,iq,chile,au,nz,ie,jp,kr,cn,hk,th,vn,ph,ng,ke,lb,jo,kw,cl,co,pe,ve,uy,latam,arab";
pub const DEFAULT_VARIANTS: &str = "raw:5,vip:2,fhd:0.6,hd:0.4,exact:5";
pub const DEFAULT_FAVOURITES: &str = "Canal+ VIP RAW,TF1,France 2,M6,L'Équipe,beIN Sports 1";

#[derive(Debug, Clone, PartialEq)]
pub struct Rules {
    pub allow_4k: bool,
    pub family_filter: bool,
    pub blocked_terms: Vec<String>,
    pub home_country: Vec<String>,
    pub home_bonus: f64,
    pub foreign_codes: Vec<String>,
    pub foreign_penalty: f64,
    pub raw: f64,
    pub vip: f64,
    pub fhd: f64,
    pub hd: f64,
    pub exact: f64,
    pub favourites: Vec<String>,
}

fn list(text: &str) -> Vec<String> {
    text.split(',').map(|part| part.trim().to_lowercase()).filter(|part| !part.is_empty()).collect()
}

fn weight(table: &str, name: &str, default: f64) -> f64 {
    table
        .split(',')
        .filter_map(|pair| pair.split_once(':'))
        .find(|(key, _)| key.trim().eq_ignore_ascii_case(name))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(default)
}

impl Default for Rules {
    fn default() -> Self {
        Rules::from_texts(&RuleTexts::default())
    }
}

pub struct RuleTexts {
    pub allow_4k: bool,
    pub family_filter: bool,
    pub blocked: String,
    pub home: String,
    pub home_bonus: f64,
    pub foreign: String,
    pub foreign_penalty: f64,
    pub variants: String,
    pub favourites: String,
}

impl Default for RuleTexts {
    fn default() -> Self {
        RuleTexts {
            allow_4k: false,
            family_filter: true,
            blocked: DEFAULT_BLOCKED.into(),
            home: "fr,fra,france".into(),
            home_bonus: 6.0,
            foreign: DEFAULT_FOREIGN.into(),
            foreign_penalty: 8.0,
            variants: DEFAULT_VARIANTS.into(),
            favourites: DEFAULT_FAVOURITES.into(),
        }
    }
}

impl Rules {
    pub fn from_texts(texts: &RuleTexts) -> Rules {
        Rules {
            allow_4k: texts.allow_4k,
            family_filter: texts.family_filter,
            blocked_terms: list(&texts.blocked),
            home_country: list(&texts.home),
            home_bonus: texts.home_bonus,
            foreign_codes: list(&texts.foreign),
            foreign_penalty: texts.foreign_penalty,
            raw: weight(&texts.variants, "raw", 5.0),
            vip: weight(&texts.variants, "vip", 2.0),
            fhd: weight(&texts.variants, "fhd", 0.6),
            hd: weight(&texts.variants, "hd", 0.4),
            exact: weight(&texts.variants, "exact", 5.0),
            favourites: texts.favourites.split(',').map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect(),
        }
    }

    pub fn read(host: &dyn HostCtx) -> Rules {
        let defaults = RuleTexts::default();
        Rules::from_texts(&RuleTexts {
            allow_4k: host.config_bool("allow4k", defaults.allow_4k),
            family_filter: host.config_bool("familyFilter", defaults.family_filter),
            blocked: host.config_str("blockedTerms", &defaults.blocked),
            home: host.config_str("homeCountry", &defaults.home),
            home_bonus: host.config_i64("homeBonus", defaults.home_bonus as i64) as f64,
            foreign: host.config_str("foreignCodes", &defaults.foreign),
            foreign_penalty: host.config_i64("foreignPenalty", defaults.foreign_penalty as i64) as f64,
            variants: host.config_str("variantWeights", &defaults.variants),
            favourites: host.config_str("favourites", &defaults.favourites),
        })
    }
}
