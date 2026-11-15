use crate::model::ProviderId;

pub const DEFAULT_USER_AGENT: &str = "VLC/3.0.20 LibVLC/3.0.20";
pub const SEGMENT_SECONDS: u32 = 1;
pub const WINDOW_SEGMENTS: u32 = 20;
pub const PLAYLIST_FILE: &str = "index.m3u8";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioMode {
    Aac,
    Copy,
}

#[derive(Debug, Clone)]
pub struct Source {
    pub name: String,
    pub url: String,
    pub provider: Option<ProviderId>,
    pub user_agent: Option<String>,
    pub realtime: bool,
    pub audio: Option<AudioMode>,
    pub hevc: bool,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub default_user_agent: String,
    pub single_connection: Vec<ProviderId>,
    pub audio_default: AudioMode,
    pub auto_copy: Vec<String>,
    pub audio_seen_copyable: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            default_user_agent: DEFAULT_USER_AGENT.into(),
            single_connection: vec![ProviderId::Backup],
            audio_default: AudioMode::Aac,
            auto_copy: Vec::new(),
            audio_seen_copyable: false,
        }
    }
}

impl Options {
    fn audio_for(&self, source: &Source) -> AudioMode {
        if let Some(mode) = source.audio {
            return mode;
        }
        let listed = self.auto_copy.iter().any(|n| n == "*" || n.eq_ignore_ascii_case(&source.name));
        if is_http(&source.url) && listed && self.audio_seen_copyable {
            return AudioMode::Copy;
        }
        self.audio_default
    }
}

pub fn is_http(url: &str) -> bool {
    url.starts_with("http")
}

pub fn input_args(source: &Source, options: &Options) -> Vec<String> {
    let mut args = Vec::new();
    if source.realtime || is_http(&source.url) {
        args.push("-re".to_string());
    }
    if is_http(&source.url) {
        args.extend(["-user_agent".to_string(), source.user_agent.clone().unwrap_or_else(|| options.default_user_agent.clone())]);
        if !source.provider.is_some_and(|p| options.single_connection.contains(&p)) {
            args.extend(["-reconnect", "1", "-reconnect_streamed", "1", "-reconnect_delay_max", "5"].map(String::from));
        }
        args.extend(["-rw_timeout", "15000000", "-analyzeduration", "3000000", "-probesize", "5000000"].map(String::from));
    }
    args
}

pub fn build(source: &Source, options: &Options, folder: &str) -> Vec<String> {
    let mut cmd: Vec<String> = ["-hide_banner", "-loglevel", "warning", "-nostdin"].map(String::from).to_vec();
    cmd.extend(input_args(source, options));
    cmd.extend(["-i".to_string(), source.url.clone()]);
    cmd.extend(["-map", "0:v:0", "-map", "0:a:0?", "-c:v", "copy"].map(String::from));
    if source.hevc {
        cmd.extend(["-tag:v", "hvc1"].map(String::from));
    }
    match options.audio_for(source) {
        AudioMode::Copy => cmd.extend(["-c:a", "copy"].map(String::from)),
        AudioMode::Aac => cmd.extend(["-c:a", "aac", "-b:a", "160k", "-ac", "2"].map(String::from)),
    }
    cmd.extend(["-progress", "pipe:1", "-stats_period", "1"].map(String::from));
    cmd.extend(
        [
            "-f".to_string(),
            "hls".into(),
            "-hls_segment_type".into(),
            "fmp4".into(),
            "-hls_time".into(),
            SEGMENT_SECONDS.to_string(),
            "-hls_list_size".into(),
            WINDOW_SEGMENTS.to_string(),
            "-hls_flags".into(),
            "delete_segments+independent_segments+program_date_time+omit_endlist".into(),
            "-hls_fmp4_init_filename".into(),
            "init.mp4".into(),
            "-hls_segment_filename".into(),
            format!("{folder}/seg%05d.m4s"),
            format!("{folder}/{PLAYLIST_FILE}"),
        ],
    );
    cmd
}

pub fn trial_args(source: &Source, options: &Options) -> Vec<String> {
    let mut cmd: Vec<String> = ["-hide_banner", "-loglevel", "warning", "-nostdin", "-re"].map(String::from).to_vec();
    cmd.extend(["-user_agent".to_string(), source.user_agent.clone().unwrap_or_else(|| options.default_user_agent.clone())]);
    cmd.extend(["-rw_timeout", "15000000", "-analyzeduration", "3000000", "-probesize", "5000000"].map(String::from));
    cmd.extend(["-i".to_string(), source.url.clone()]);
    cmd.extend(["-map", "0:v:0", "-map", "0:a:0?", "-c", "copy", "-progress", "pipe:1", "-stats_period", "1", "-f", "null", "-"].map(String::from));
    cmd
}

pub fn restart_delay_seconds(restarts: u32) -> u64 {
    (1u64 << restarts.min(5)).min(30)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMARY: &str = "http://portal.test:80/TESTUSER/TESTPASS42/1.ts";

    fn source(url: &str, provider: Option<ProviderId>) -> Source {
        Source { name: "a".into(), url: url.into(), provider, user_agent: None, realtime: false, audio: None, hevc: false }
    }

    fn value<'a>(cmd: &'a [String], flag: &str) -> &'a str {
        &cmd[cmd.iter().position(|a| a == flag).unwrap() + 1]
    }

    #[test]
    fn video_is_copied_audio_is_aac_and_progress_follows_the_input() {
        let cmd = build(&source(PRIMARY, Some(ProviderId::Primary)), &Options::default(), "/pub/run");

        assert_eq!((value(&cmd, "-c:v"), value(&cmd, "-c:a"), value(&cmd, "-b:a")), ("copy", "aac", "160k"));
        let progress = cmd.iter().position(|a| a == "-progress").unwrap();
        assert_eq!(&cmd[progress + 1..progress + 4], ["pipe:1", "-stats_period", "1"]);
        assert!(cmd.iter().position(|a| a == "-i").unwrap() < progress);
        assert_eq!(cmd.last().unwrap(), "/pub/run/index.m3u8");
    }

    #[test]
    fn the_output_is_a_sliding_fragmented_mp4_playlist_with_program_date_time() {
        let cmd = build(&source(PRIMARY, None), &Options::default(), "/pub/run");

        assert_eq!(value(&cmd, "-f"), "hls");
        assert_eq!(value(&cmd, "-hls_segment_type"), "fmp4");
        assert_eq!(value(&cmd, "-hls_list_size"), "20");
        assert_eq!(value(&cmd, "-hls_time"), "1");
        let flags = value(&cmd, "-hls_flags");
        for flag in ["delete_segments", "independent_segments", "program_date_time", "omit_endlist"] {
            assert!(flags.contains(flag), "{flags}");
        }
        assert_eq!(value(&cmd, "-hls_segment_filename"), "/pub/run/seg%05d.m4s");
    }

    #[test]
    fn audio_is_copied_on_request_or_for_a_listed_channel_whose_audio_was_seen_copyable() {
        let mut copy = source(PRIMARY, None);
        copy.audio = Some(AudioMode::Copy);
        assert_eq!(value(&build(&copy, &Options::default(), "/p"), "-c:a"), "copy");
        let mut options = Options { auto_copy: vec!["a".into()], ..Options::default() };
        assert_eq!(value(&build(&source(PRIMARY, None), &options, "/p"), "-c:a"), "aac");
        options.audio_seen_copyable = true;
        assert_eq!(value(&build(&source(PRIMARY, None), &options, "/p"), "-c:a"), "copy");
        options.auto_copy = vec!["*".into()];
        assert_eq!(value(&build(&source(PRIMARY, None), &options, "/p"), "-c:a"), "copy");
        options.auto_copy = vec!["other".into()];
        assert_eq!(value(&build(&source(PRIMARY, None), &options, "/p"), "-c:a"), "aac");
    }

    #[test]
    fn realtime_applies_to_http_or_on_request_and_a_file_without_it_is_not_paced() {
        assert!(build(&source(PRIMARY, None), &Options::default(), "/p").contains(&"-re".to_string()));
        assert!(!build(&source("/tmp/x.ts", None), &Options::default(), "/p").contains(&"-re".to_string()));
        let mut paced = source("/tmp/x.ts", None);
        paced.realtime = true;
        assert!(build(&paced, &Options::default(), "/p").contains(&"-re".to_string()));
    }

    #[test]
    fn the_backup_never_reconnects_by_itself_because_it_would_open_a_second_connection() {
        let backup = build(&source(PRIMARY, Some(ProviderId::Backup)), &Options::default(), "/p");
        let primary = build(&source(PRIMARY, Some(ProviderId::Primary)), &Options::default(), "/p");

        assert!(!backup.contains(&"-reconnect".to_string()));
        assert!(primary.contains(&"-reconnect".to_string()));
        assert_eq!(value(&backup, "-rw_timeout"), "15000000");
        let relaxed = Options { single_connection: Vec::new(), ..Options::default() };
        assert!(build(&source(PRIMARY, Some(ProviderId::Backup)), &relaxed, "/p").contains(&"-reconnect".to_string()));
    }

    #[test]
    fn the_providers_own_user_agent_wins_over_the_default() {
        let mut own = source(PRIMARY, None);
        own.user_agent = Some("IPTVSmartersPro".into());

        assert_eq!(value(&build(&own, &Options::default(), "/p"), "-user_agent"), "IPTVSmartersPro");
        assert_eq!(value(&build(&source(PRIMARY, None), &Options::default(), "/p"), "-user_agent"), DEFAULT_USER_AGENT);
    }

    #[test]
    fn hevc_video_is_tagged_hvc1_for_apple_players_and_h264_is_left_alone() {
        let mut hevc = source(PRIMARY, None);
        hevc.hevc = true;

        let tagged = build(&hevc, &Options::default(), "/p");
        let plain = build(&source(PRIMARY, None), &Options::default(), "/p");

        let at = tagged.iter().position(|a| a == "-tag:v").unwrap();
        assert_eq!(tagged[at + 1], "hvc1");
        assert_eq!(tagged[at - 2..at], ["-c:v", "copy"]);
        assert!(!plain.contains(&"-tag:v".to_string()));
    }

    #[test]
    fn a_trial_copies_to_nothing_and_never_reconnects() {
        let cmd = trial_args(&source(PRIMARY, Some(ProviderId::Backup)), &Options::default());

        assert!(!cmd.contains(&"-reconnect".to_string()));
        assert_eq!(&cmd[cmd.len() - 3..], ["-f", "null", "-"]);
        assert_eq!(value(&cmd, "-c"), "copy");
    }

    #[test]
    fn a_restart_waits_twice_as_long_each_time_up_to_thirty_seconds() {
        let delays: Vec<u64> = (1..=9).map(restart_delay_seconds).collect();

        assert_eq!(delays, [2, 4, 8, 16, 30, 30, 30, 30, 30]);
    }
}
