ALTER TABLE files ADD COLUMN v_dv_level INTEGER;
ALTER TABLE files ADD COLUMN v_dv_compat INTEGER;
ALTER TABLE files ADD COLUMN v_dv_el INTEGER;

ALTER TABLE files ADD COLUMN probe_schema INTEGER NOT NULL DEFAULT 0;

UPDATE files SET probe_schema = 1
 WHERE NOT (probed = 1 AND unreadable IS NULL
   AND (v_hdr = 1
        OR v_dv_profile IS NOT NULL
        OR a_codec IN ('eac3', 'truehd', 'dts')
        OR audio_tracks LIKE '%"codec":"eac3"%'
        OR audio_tracks LIKE '%"codec":"truehd"%'
        OR audio_tracks LIKE '%"codec":"dts"%'));
