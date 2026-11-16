//! M4: animation attributes and the timeline (FR-ANIM-01, FR-ANIM-02, FR-ANIM-04).

use s_md_slides::{Deck, Phase, State};

const SLIDE: &str = "<!-- text id=title x=100 y=100 w=800 h=100 -->\n# Always there\n\n<!-- text id=a x=100 y=300 w=400 h=100 enter=fly-left delay=300 dur=500 order=1 -->\nA\n<!-- text id=b x=600 y=300 w=400 h=100 enter=fade order=1 -->\nB\n<!-- shape id=c kind=rect x=100 y=500 w=100 h=100 enter=scale order=2 exit=fade -->\n<!-- text id=auto x=0 y=0 w=10 h=10 enter=appear -->\nauto\n";

#[test]
fn fr_anim_01_effect_attributes_parse_and_round_trip() {
    let d = Deck::parse(SLIDE).deck;
    assert!(d.diagnostics().is_empty(), "{:?}", d.diagnostics());
    assert_eq!(d.text(), SLIDE);
    let a = &d.element("a").unwrap().anim;
    assert_eq!(
        (a.enter.as_deref(), a.delay, a.dur, a.order),
        (Some("fly-left"), 300, 500, Some(1))
    );
    let bad =
        Deck::parse("<!-- text id=x x=0 y=0 w=1 h=1 enter=spin order=0 dur=-1 -->\n").diagnostics;
    assert_eq!(bad.iter().filter(|d| d.code == "E-ATTR-VALUE").count(), 3);
}

#[test]
fn fr_anim_02_orders_1_1_2_take_two_clicks() {
    let d = Deck::parse(SLIDE).deck;
    let t = d.timeline(0).unwrap();
    assert_eq!(
        t.clicks(),
        3,
        "order 1, order 2, then c's exit on the click after"
    );
    let ids = |k: usize| {
        t.steps[k]
            .entries
            .iter()
            .map(|e| (e.id.as_str(), e.phase))
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(0), [("auto", Phase::Enter)]);
    assert_eq!(ids(1), [("a", Phase::Enter), ("b", Phase::Enter)]);
    assert_eq!(ids(2), [("c", Phase::Enter)]);
    assert_eq!(ids(3), [("c", Phase::Exit)]);
    let two = Deck::parse("<!-- text id=a x=0 y=0 w=1 h=1 enter=fade order=1 -->\n<!-- text id=b x=0 y=0 w=1 h=1 enter=fade order=1 -->\n<!-- text id=c x=0 y=0 w=1 h=1 enter=fade order=2 -->\n").deck;
    assert_eq!(two.timeline(0).unwrap().clicks(), 2);
}

#[test]
fn fr_anim_04_state_at() {
    let d = Deck::parse(SLIDE).deck;
    let t = d.timeline(0).unwrap();
    // Step 0 is `appear` (instant), so step 1 starts at 0 and lasts delay + dur = 800.
    assert_eq!((t.steps[1].start, t.steps[1].duration), (0, 800));
    assert_eq!(t.duration, 800 + 400 + 400);
    let s0 = t.state_at(0.0);
    assert!(!s0.contains_key("title"), "static elements are not listed");
    assert_eq!(
        s0["a"],
        State {
            opacity: 1.0,
            dx: -500.0,
            dy: 0.0,
            scale: 1.0
        }
    );
    assert_eq!(s0["b"].opacity, 0.0);
    assert_eq!(s0["auto"], State::VISIBLE);
    // Halfway through a's 500 ms: ease-out cubic of 0.5 is 0.875.
    let mid = t.state_at(300.0 + 250.0);
    assert!((mid["a"].dx - (-500.0 * 0.125)).abs() < 1e-9);
    let end = t.state_at(t.duration as f64);
    assert_eq!(end["a"], State::VISIBLE);
    assert_eq!(end["c"].opacity, 0.0, "c has exited");
    assert_eq!(t.state_at(t.end_of_step(2) as f64)["c"], State::VISIBLE);
    assert!(
        Deck::parse("# no animation\n")
            .deck
            .timeline(0)
            .unwrap()
            .steps[0]
            .entries
            .is_empty()
    );
}

#[test]
fn state_at_step_keeps_zero_length_steps_apart() {
    let d = Deck::parse("<!-- text id=a x=0 y=0 w=1 h=1 enter=appear order=1 -->\n<!-- text id=b x=0 y=0 w=1 h=1 enter=appear order=2 -->\n").deck;
    let t = d.timeline(0).unwrap();
    assert_eq!(t.duration, 0, "appear steps take no time");
    assert_eq!(
        t.state_at(0.0)["b"].opacity,
        1.0,
        "so state_at(0) already shows b"
    );
    let s1 = t.state_at_step(1, 0.0);
    assert_eq!(
        (s1["a"].opacity, s1["b"].opacity),
        (1.0, 0.0),
        "step 1 shows a, not yet b"
    );
    assert_eq!(t.state_at_step(2, 0.0)["b"].opacity, 1.0);
}
