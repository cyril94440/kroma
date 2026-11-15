use super::model::{Effect, ResultRow, Status};
use super::score::points;
use super::state::{
    in_window, Pronos, BEFORE_KICKOFF_S, KEEP_FINAL_S, MAX_DURATION_S, MISSED_MAX, ZAPPED_AFTER_S,
};
use crate::detect::FlatMatch;

impl Pronos {
    pub fn follow(&mut self, found: Option<&FlatMatch>, source_ok: bool, now: f64) -> Vec<Effect> {
        let Some(current) = self.current.as_mut().filter(|c| c.status != Status::Final) else {
            return Vec::new();
        };
        let mut effects = Vec::new();
        if let Some(found) = found {
            current.missed = 0;
            if (&current.score, &current.minute) != (&found.score, &found.minute) {
                current.score = found.score.clone();
                current.minute = found.minute.clone();
                effects.push(Effect::Updated);
            }
            if current.status == Status::Open
                && matches!(found.state.as_str(), "in" | "post")
                && !current.opened_mid_match
            {
                current.closes_at = current.closes_at.min(now);
            }
        } else if source_ok {
            current.missed += 1;
        }
        if current.status == Status::Open && now >= current.closes_at {
            current.status = Status::Running;
            effects.push(Effect::Closed);
        }
        let ended = found.is_some_and(|f| f.state == "post");
        let lost = current.missed >= MISSED_MAX || now - current.kickoff > MAX_DURATION_S;
        if ended {
            if current.status == Status::Open {
                current.status = Status::Running;
            }
            self.finalise(now);
            effects.push(Effect::Finished);
        } else if lost {
            let mut gone = self.current.take().unwrap();
            gone.status = Status::Final;
            gone.ended_at = Some(now);
            gone.counted = true;
            gone.results.clear();
            self.cancelled = Some(gone);
            effects.push(Effect::Abandoned);
        }
        self.dirty = true;
        effects
    }

    fn finalise(&mut self, now: f64) {
        let Some(current) = self.current.as_mut() else {
            return;
        };
        if current.counted {
            return;
        }
        current.counted = true;
        current.status = Status::Final;
        current.ended_at = Some(now);
        current.closes_at = current.closes_at.min(now);
        let mut rows = Vec::new();
        for (user, guess) in &current.votes {
            let earned = points(guess, &current.score);
            let name = current
                .names
                .get(user)
                .cloned()
                .unwrap_or_else(|| user.clone());
            rows.push(ResultRow {
                user_id: user.clone(),
                pseudo: name.clone(),
                prono: guess.clone(),
                points: earned,
                exact: earned == 3,
            });
            let season = self.season.entry(user.clone()).or_default();
            season.name = name;
            season.points += earned;
            season.won += u32::from(earned == 3);
        }
        rows.sort_by_key(|r| (std::cmp::Reverse(r.points), r.pseudo.to_lowercase()));
        current.results = rows;
    }

    pub fn purge(&mut self, now: f64) {
        if self
            .cancelled
            .as_ref()
            .is_some_and(|c| now - c.ended_at.unwrap_or(now) > KEEP_FINAL_S)
        {
            self.cancelled = None;
        }
        if self.current.as_ref().is_some_and(|c| {
            c.status == Status::Final && now - c.ended_at.unwrap_or(now) > KEEP_FINAL_S
        }) {
            self.current = None;
            self.dirty = true;
        }
    }

    fn away(&mut self, channel: &str, direct_id: Option<&str>, now: f64) -> bool {
        let Some(current) = self.current.as_mut() else {
            return false;
        };
        let elsewhere = !channel.is_empty()
            && channel != current.channel
            && direct_id != Some(current.id.as_str());
        if !current.votes.is_empty() || !current.auto || !elsewhere {
            current.away_since = None;
            return false;
        }
        let since = *current.away_since.get_or_insert(now);
        if direct_id.is_none() && now - since < ZAPPED_AFTER_S {
            return false;
        }
        let mut gone = self.current.take().unwrap();
        gone.status = Status::Final;
        gone.ended_at = Some(now);
        gone.counted = true;
        gone.results.clear();
        self.seen.retain(|s| *s != gone.id);
        self.cancelled = Some(gone);
        self.dirty = true;
        true
    }

    pub fn cycle(
        &mut self,
        matches: Option<&[FlatMatch]>,
        detected: Option<&FlatMatch>,
        channel: &str,
        required: &[(String, String)],
        now: f64,
    ) -> Vec<Effect> {
        self.purge(now);
        if self.active() && self.away(channel, detected.map(|d| d.id.as_str()), now) {
            return vec![Effect::Away];
        }
        if let Some(current) = self.current.as_ref().filter(|c| c.status != Status::Final) {
            let found = matches.and_then(|m| m.iter().find(|x| x.id == current.id));
            return self.follow(found, matches.is_some(), now);
        }
        let opens =
            detected.filter(|d| in_window(d, now, BEFORE_KICKOFF_S) && !self.is_seen(&d.id));
        match opens {
            Some(found) if self.open(found, channel, required, true, now) => vec![Effect::Opened],
            _ => Vec::new(),
        }
    }
}
