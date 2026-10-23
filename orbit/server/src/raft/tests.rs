use super::core::*;
use super::sim::*;

fn chaos() -> Faults {
    Faults {
        drop_percent: 10,
        dup_percent: 5,
        max_delay: 4,
        crash_percent_per_tick_per_1000: 4,
        partition_every: 60,
    }
}

#[test]
fn a_cluster_elects_one_leader_and_commits_in_order() {
    for n in [1u64, 3, 5] {
        let mut sim = Sim::new(n, 7);
        sim.run(200, 5);
        assert!(sim.leader().is_some(), "{n} nodes elect a leader");
        sim.heal();
        sim.assert_converged(100);
        assert!(
            sim.acked_count() > 10,
            "{n} nodes acknowledged {} writes",
            sim.acked_count()
        );
    }
}

/// `RAFT_SIM_SEEDS` raises the number of random runs (default 40) and `RAFT_SIM_CRASHY=1` crashes nodes far more often.
fn seeds() -> u64 {
    std::env::var("RAFT_SIM_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40)
}

#[test]
fn safety_and_liveness_hold_under_loss_partitions_and_crashes() {
    let crashy = std::env::var("RAFT_SIM_CRASHY").is_ok();
    let mut worst_acked = usize::MAX;
    for seed in 1..=seeds() {
        let nodes = if seed % 2 == 0 { 3 } else { 5 };
        let mut sim = Sim::new(nodes, seed * 7919);
        let mut f = chaos();
        if crashy {
            f.crash_percent_per_tick_per_1000 = 25;
            f.partition_every = 25;
            f.drop_percent = 20;
        }
        sim.set_faults(f);
        sim.run(if crashy { 900 } else { 1500 }, 3);
        sim.heal();
        sim.assert_converged(400);
        worst_acked = worst_acked.min(sim.acked_count());
    }
    assert!(
        worst_acked > 0 || crashy,
        "every run made progress at least once"
    );
}

#[test]
fn a_lagging_node_catches_up_from_a_snapshot() {
    let mut sim = Sim::new(3, 99);
    sim.run(100, 4);
    let leader = sim.leader().expect("leader");
    // cut one follower off, write a lot, and compact so the log it needs is gone
    let lagging = (1..=3u64).find(|id| *id != leader).unwrap();
    sim.isolate(lagging);
    sim.run(400, 2);
    sim.compact_all(5);
    sim.run(100, 2);
    sim.heal();
    sim.assert_converged(300);
}

#[test]
fn membership_changes_one_server_at_a_time() {
    let mut sim = Sim::new(3, 5);
    sim.run(100, 4);
    assert!(sim.add_voter(4));
    sim.run(200, 4);
    assert_eq!(sim.voters_of_leader().len(), 4);
    assert!(sim.add_voter(5));
    sim.run(200, 4);
    assert_eq!(sim.voters_of_leader().len(), 5);
    let leader = sim.leader().unwrap();
    // remove a follower, then the leader itself
    let follower = (1..=5u64).find(|id| *id != leader).unwrap();
    assert!(sim.remove_voter(follower));
    sim.run(200, 4);
    assert_eq!(sim.voters_of_leader().len(), 4);
    let leader = sim.leader().unwrap();
    assert!(sim.remove_voter(leader));
    sim.run(400, 4);
    assert_eq!(sim.voters_of_leader().len(), 3);
    assert!(!sim.voters_of_leader().contains(&leader));
    sim.heal();
    sim.assert_converged(300);
}

#[test]
fn membership_changes_survive_chaos() {
    for seed in 1..=40u64 {
        let mut sim = Sim::new(3, seed * 31337);
        sim.set_faults(chaos());
        sim.run(200, 3);
        sim.add_voter(4);
        sim.run(300, 3);
        sim.add_voter(5);
        sim.run(300, 3);
        sim.heal();
        sim.assert_converged(500);
    }
}

#[test]
fn only_one_membership_change_may_be_in_flight() {
    let mut r = Raft::new(
        1,
        vec![1],
        HardState::default(),
        None,
        vec![],
        0,
        Options::default(),
        1,
    );
    for _ in 0..50 {
        r.tick();
    }
    assert!(r.is_leader());
    assert!(r.propose_config(vec![1, 2]).is_ok());
    assert!(matches!(
        r.propose_config(vec![1, 2, 3]),
        Err(ProposeError::BadConfigChange(_))
    ));
    assert!(matches!(
        Raft::new(
            2,
            vec![1, 2, 3],
            HardState::default(),
            None,
            vec![],
            0,
            Options::default(),
            1
        )
        .propose(vec![1]),
        Err(ProposeError::NotLeader { .. })
    ));
}

fn env(from: u64, to: u64, term: u64, body: Body) -> Envelope {
    Envelope {
        from,
        to,
        term,
        body,
    }
}

/// Raft's "Figure 8": an entry from an earlier term that sits on a majority is still not committed,
/// because a later leader could overwrite it. It becomes committed only together with an entry of the
/// leader's own term.
#[test]
fn a_leader_does_not_commit_an_older_term_entry_by_counting_replicas() {
    let config = Entry {
        term: 1,
        index: 1,
        payload: Payload::Config(vec![1, 2, 3]),
    };
    let old = Entry {
        term: 2,
        index: 2,
        payload: Payload::Data(b"old".to_vec()),
    };
    let mut r = Raft::new(
        1,
        vec![1, 2, 3],
        HardState {
            term: 3,
            voted_for: None,
        },
        None,
        vec![config, old],
        0,
        Options::default(),
        1,
    );
    // run until it asks for pre-votes, grant one, then grant a real vote
    for _ in 0..100 {
        if r.role() == Role::PreCandidate {
            break;
        }
        r.tick();
    }
    assert_eq!(r.role(), Role::PreCandidate);
    let _ = r.ready();
    r.step(env(2, 1, 4, Body::PreVoteReply { granted: true }));
    assert_eq!(r.role(), Role::Candidate);
    assert_eq!(r.term(), 4);
    let _ = r.ready();
    r.step(env(2, 1, 4, Body::VoteReply { granted: true }));
    assert!(r.is_leader());
    assert_eq!(
        r.last_index(),
        3,
        "the new leader appends a no-op of its own term"
    );
    let _ = r.ready();
    // node 2 reports it holds the old entry: that is a majority for index 2, but not a commit
    r.step(env(
        2,
        1,
        4,
        Body::AppendReply {
            success: true,
            match_index: 2,
            hint: 0,
        },
    ));
    assert!(
        r.commit_index() < 2,
        "an entry of an earlier term must not be committed by counting replicas, commit = {}",
        r.commit_index()
    );
    // once the leader's own entry is on a majority, everything up to it commits
    r.step(env(
        2,
        1,
        4,
        Body::AppendReply {
            success: true,
            match_index: 3,
            hint: 0,
        },
    ));
    assert_eq!(r.commit_index(), 3);
}

/// A server must not vote for a candidate whose log is behind its own.
#[test]
fn a_vote_is_refused_to_a_candidate_with_an_older_log() {
    let es = vec![
        Entry {
            term: 1,
            index: 1,
            payload: Payload::Config(vec![1, 2, 3]),
        },
        Entry {
            term: 2,
            index: 2,
            payload: Payload::Data(vec![1]),
        },
    ];
    let mut r = Raft::new(
        1,
        vec![1, 2, 3],
        HardState {
            term: 2,
            voted_for: None,
        },
        None,
        es,
        0,
        Options::default(),
        1,
    );
    r.step(env(
        2,
        1,
        3,
        Body::Vote {
            last_index: 1,
            last_term: 1,
        },
    ));
    let out = r.ready();
    assert!(out
        .messages
        .iter()
        .any(|m| matches!(m.body, Body::VoteReply { granted: false })));
    r.step(env(
        3,
        1,
        3,
        Body::Vote {
            last_index: 2,
            last_term: 2,
        },
    ));
    let out = r.ready();
    assert!(out
        .messages
        .iter()
        .any(|m| matches!(m.body, Body::VoteReply { granted: true })));
    // one vote per term
    r.step(env(
        2,
        1,
        3,
        Body::Vote {
            last_index: 5,
            last_term: 2,
        },
    ));
    let out = r.ready();
    assert!(out
        .messages
        .iter()
        .any(|m| matches!(m.body, Body::VoteReply { granted: false })));
}
