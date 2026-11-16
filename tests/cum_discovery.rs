use personal_downloaders::{Cancel, Site, sites::cum, state::State};
use serde_json::{Value, json};
use std::{cell::RefCell, future::ready};
use url::Url;
const PROFILE: &str = "https://cum.st/creators/fansly/999000000000000001";
fn asset(n: u64) -> Value {
    json!({"id":format!("asset-{n}"),"sha256":format!("{n:064x}"),"storageKey":format!("{n:064x}"),"kind":"image","mimeType":"image/jpeg","originalFilename":"../bad.jpeg","bytes":4,"variants":[{"name":"preview.webp","bytes":1},{"name":"original.jpeg","bytes":4}]})
}
fn post(n: u64) -> Value {
    json!({"id":n.to_string(),"service":"fansly","published":n,"attachments":[asset(n)]})
}
fn profile() -> Value {
    json!({"id":"999000000000000001","service":"fansly","name":"niwwu"})
}
async fn resolve(
    state: &State,
    refresh: bool,
    update: bool,
    total: usize,
    posts: Vec<Value>,
) -> anyhow::Result<personal_downloaders::Listing> {
    cum::resolve(state,PROFILE,refresh,update,&Cancel::default(),&|url:String|ready(Ok(if url.ends_with("profile"){profile()}else{let u=Url::parse(&url).unwrap();let o=u.query_pairs().find(|(k,_)|k=="o").unwrap().1.parse::<usize>().unwrap();json!({"total":total,"posts":posts.iter().skip(o).take(50).cloned().collect::<Vec<_>>()})}))).await
}
#[test]
fn presentation_parameters_normalize_without_accepting_content_filters() {
    assert_eq!(
        cum::normalize(&format!("{PROFILE}?display=collages&ppsort=popular")).unwrap(),
        PROFILE
    );
    for bad in [
        "http://cum.st/creators/fansly/1",
        "https://cum.st.evil.test/creators/fansly/1",
        "https://u@cum.st/creators/fansly/1",
        "https://cum.st/creators/fansly/1?ppq=x",
        "https://cum.st/creators/fansly/1?tab=dms",
        "https://cum.st/creators/fansly/1?pptype=videos",
    ] {
        assert!(cum::normalize(bad).is_err(), "{bad}");
    }
}
#[tokio::test]
async fn originals_are_declared_hash_bound_and_not_collages_or_previews() {
    let root = tempfile::tempdir().unwrap();
    let state = State::open(root.path(), Site::Cum).unwrap();
    let listing = resolve(&state, false, false, 2, vec![post(2), post(1)])
        .await
        .unwrap();
    assert_eq!(listing.files.len(), 2);
    assert_eq!(listing.title, "niwwu [fansly-999000000000000001]");
    let f = &listing.files[0];
    assert!(f.url.ends_with("/original.jpeg"));
    assert_eq!(f.sha256, format!("{:064x}", 2));
    assert_eq!(f.expected_size, Some(4));
    assert!(!f.path.contains("bad"));
    assert!(
        f.fallback
            .as_ref()
            .unwrap()
            .path
            .ends_with(".thumbnail.webp")
    );
    assert!(f.fallback.as_ref().unwrap().sha256.is_empty());
    assert!(cum::validate_media(&Url::parse(&f.url).unwrap(), PROFILE).is_ok());
    assert!(
        cum::validate_media(
            &Url::parse(&f.url.replace("cdn.cum.st", "cdn.cum.st.evil.test")).unwrap(),
            PROFILE
        )
        .is_err()
    );
    assert!(
        cum::validate_redirect(
            &Url::parse(&f.fallback.as_ref().unwrap().url).unwrap(),
            &Url::parse(&f.url).unwrap(),
            PROFILE
        )
        .is_err()
    );
    let cached = cum::resolve(&state, PROFILE, false, false, &Cancel::default(), &|_| {
        ready(Err(anyhow::anyhow!("cached list must not fetch")))
    })
    .await
    .unwrap();
    assert_eq!(cached.files.len(), 2);
}
#[tokio::test]
async fn full_pagination_and_unchanged_frontier_preserve_history() {
    let root = tempfile::tempdir().unwrap();
    let state = State::open(root.path(), Site::Cum).unwrap();
    let posts = (1..=51).rev().map(post).collect::<Vec<_>>();
    resolve(&state, false, false, 51, posts.clone())
        .await
        .unwrap();
    let reads = RefCell::new(Vec::new());
    let again = cum::resolve(
        &state,
        PROFILE,
        false,
        true,
        &Cancel::default(),
        &|url: String| {
            reads.borrow_mut().push(url.clone());
            ready(Ok(if url.ends_with("profile") {
                profile()
            } else {
                json!({"total":51,"posts":posts[..50]})
            }))
        },
    )
    .await
    .unwrap();
    assert_eq!(again.files.len(), 51);
    assert_eq!(reads.borrow().len(), 2);
    let mut changed = vec![post(52)];
    changed.extend(posts[..50].iter().cloned());
    let updated = resolve(&state, false, true, 51, changed).await.unwrap();
    assert_eq!(updated.files.len(), 52);
}
#[tokio::test]
async fn invalid_or_cancelled_updates_do_not_replace_a_complete_checkpoint() {
    let root = tempfile::tempdir().unwrap();
    let state = State::open(root.path(), Site::Cum).unwrap();
    resolve(&state, false, false, 1, vec![post(1)])
        .await
        .unwrap();
    let before = state.post_snapshot(PROFILE).unwrap();
    let mut foreign = post(2);
    foreign["service"] = json!("onlyfans");
    assert!(
        resolve(&state, false, true, 1, vec![foreign])
            .await
            .is_err()
    );
    assert_eq!(state.post_snapshot(PROFILE).unwrap(), before);
    assert!(
        resolve(&state, false, true, 2, vec![post(2)])
            .await
            .is_err()
    );
    assert_eq!(state.post_snapshot(PROFILE).unwrap(), before);
    let cancel = Cancel::default();
    cancel.cancel();
    assert!(
        cum::resolve(&state, PROFILE, false, true, &cancel, &|_| ready(Ok(
            profile()
        )))
        .await
        .is_err()
    );
    assert_eq!(state.post_snapshot(PROFILE).unwrap(), before);
}

#[test]
fn api_lone_surrogates_are_repaired_without_changing_escaped_literals() {
    let value = cum::parse_json(br#"{"caption":"hi \ud83d", "low":"\ude00", "pair":"\ud83d\ude00", "literal":"\\ud83d", "quote":"\""}"#).unwrap();
    assert_eq!(value["caption"], "hi \u{fffd}");
    assert_eq!(value["low"], "\u{fffd}");
    assert_eq!(value["pair"], "\u{1f600}");
    assert_eq!(value["literal"], r"\ud83d");
    assert!(cum::parse_json(br#"{"x":garbage}"#).is_err());
}
#[tokio::test]
async fn conflicting_original_metadata_preserves_checkpoint() {
    let root = tempfile::tempdir().unwrap();
    let state = State::open(root.path(), Site::Cum).unwrap();
    resolve(&state, false, false, 1, vec![post(1)])
        .await
        .unwrap();
    let before = state.post_snapshot(PROFILE).unwrap();
    for kind in 0..3 {
        let mut next = post(2);
        let a = &mut next["attachments"][0];
        match kind {
            0 => a["storageKey"] = json!(format!("{:064x}", 3)),
            1 => a["bytes"] = json!(8),
            _ => a["variants"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name":"original.jpeg","bytes":8})),
        }
        assert!(resolve(&state, false, true, 1, vec![next]).await.is_err());
        assert_eq!(state.post_snapshot(PROFILE).unwrap(), before);
    }
}
#[tokio::test]
async fn missing_timestamp_keeps_frontier_disabled_for_later_pages() {
    let root = tempfile::tempdir().unwrap();
    let state = State::open(root.path(), Site::Cum).unwrap();
    let posts = (1..=101).rev().map(post).collect::<Vec<_>>();
    resolve(&state, false, false, 101, posts.clone())
        .await
        .unwrap();
    let mut changed = posts.clone();
    changed[0].as_object_mut().unwrap().remove("published");
    changed[100] = post(102);
    changed[100]["published"] = json!(1);
    let result = resolve(&state, false, true, 101, changed).await.unwrap();
    assert_eq!(result.files.len(), 102);
}

#[tokio::test]
async fn retrying_pending_metadata_keeps_historical_files() {
    let root = tempfile::tempdir().unwrap();
    let state = State::open(root.path(), Site::Cum).unwrap();
    resolve(&state, false, false, 2, vec![post(2), post(1)])
        .await
        .unwrap();
    let mut unavailable = post(3);
    unavailable["attachments"][0]
        .as_object_mut()
        .unwrap()
        .remove("variants");
    let pending = resolve(&state, false, true, 2, vec![unavailable, post(2)])
        .await
        .unwrap();
    assert_eq!(pending.pending.len(), 1);
    assert_eq!(pending.files.len(), 2);
    let retried = resolve(&state, false, true, 2, vec![post(3), post(2)])
        .await
        .unwrap();
    assert!(retried.pending.is_empty());
    assert_eq!(retried.files.len(), 3);
}
