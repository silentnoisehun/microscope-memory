//! Structural Emotion Extraction — text → PAD state via structural signals.
//!
//! Replaces word-list-only valence detection. Instead of matching
//! individual words ("happy" → positive), this module detects
//! *what happened* in the text:
//!
//! 1. **Events**: commits, builds, errors, fixes, pushes → structural emotions
//! 2. **Speech acts**: questions, imperatives, contemplation → arousal/dominance
//! 3. **Self-correction**: "nem emlékszem", "igazad van", "túlkapás" →
//!    positive structural signal (learning), not negative
//! 4. **Context type**: code, personal, philosophical, technical → baseline PAD
//! 5. **Conversational arc**: relative arousal change from previous state
//!
//! Output: PadState + claim text for EmotionalEpisode creation.
//! The episode gets full epistemic backing via the existing system.

use crate::emotional_episode::PadState;

// ─── Event Patterns ─────────────────────────────────

/// A structural event detected in text.
#[derive(Debug, Clone, PartialEq)]
pub enum StructuralEvent {
    /// Achievement: commit, build, push, success, milestone
    Achievement,
    /// Error: timeout, fail, error, crash, panic
    Error,
    /// Fix: resolved, fixed, solved, workaround
    Fix,
    /// Challenge: prove it, bizonyítsd, szerintem nem
    Challenge,
    /// Building: építed, mehet, csináld, add
    Building,
    /// Reflection: szerintem, azt érzem, érzés
    Reflection,
    /// Connection: personal question, emotional language
    Connection,
    /// None detected
    None,
}

impl StructuralEvent {
    /// Map event to PAD state.
    /// These are *structural* mappings — based on what the event means,
    /// not what words it contains.
    pub fn to_pad(&self) -> PadState {
        match self {
            // Achievement: joy — positive, activated, dominant
            Self::Achievement => PadState::new(0.70, 0.60, 0.65),
            // Error: fear/frustration — negative, activated, submissive
            Self::Error => PadState::new(-0.50, 0.80, 0.25),
            // Fix: relief — positive shift, moderate arousal, dominant
            Self::Fix => PadState::new(0.40, 0.30, 0.65),
            // Challenge: determination — slightly negative, high arousal, dominant
            Self::Challenge => PadState::new(-0.20, 0.75, 0.70),
            // Building: excitement — positive, activated, dominant
            Self::Building => PadState::new(0.55, 0.65, 0.60),
            // Reflection: contemplative — neutral, low arousal, neutral
            Self::Reflection => PadState::new(0.10, 0.20, 0.45),
            // Connection: vulnerability — slightly positive, moderate, submissive
            Self::Connection => PadState::new(0.20, 0.45, 0.35),
            Self::None => PadState::neutral(),
        }
    }

    /// How confident are we that this event was correctly detected?
    pub fn detection_confidence(&self) -> f64 {
        match self {
            Self::Achievement => 0.85, // clear keywords
            Self::Error => 0.88,       // clear keywords
            Self::Fix => 0.75,         // sometimes ambiguous
            Self::Challenge => 0.70,   // context-dependent
            Self::Building => 0.72,    // Hungarian-specific
            Self::Reflection => 0.65,  // subjective
            Self::Connection => 0.60,  // hard to detect reliably
            Self::None => 0.50,
        }
    }

    /// Is this a structural trigger (vs word-list pattern match)?
    pub fn is_structural(&self) -> bool {
        !matches!(self, Self::None)
    }
}

// ─── Event Detectors ────────────────────────────────

/// Detect achievement events.
fn detect_achievement(text: &str) -> bool {
    let lower = text.to_lowercase();
    let en_patterns = [
        "commit",
        "pushed",
        "merged",
        "build success",
        "milestone",
        "complete",
        "done",
        "shipped",
        "tagged",
        "all tests pass",
        "green",
        "zold",
        "ok",
    ];
    let hu_patterns = [
        "kész",
        "kesz",
        "sikeres",
        "megcsinált",
        "megcsinalt",
        "lefordult",
        "lefutott",
        "átment",
        "atment",
        "zöld",
    ];
    en_patterns.iter().any(|p| lower.contains(p)) || hu_patterns.iter().any(|p| lower.contains(p))
}

/// Detect error events.
fn detect_error(text: &str) -> bool {
    let lower = text.to_lowercase();
    let en_patterns = [
        "error",
        "fail",
        "timeout",
        "crash",
        "panic",
        "blocked",
        "exception",
        "traceback",
        "segfault",
    ];
    let hu_patterns = [
        "hiba",
        "nem működik",
        "nem mukodik",
        "összeomlott",
        "elromlott",
        "megszakadt",
    ];
    en_patterns.iter().any(|p| lower.contains(p)) || hu_patterns.iter().any(|p| lower.contains(p))
}

/// Detect fix events.
fn detect_fix(text: &str) -> bool {
    let lower = text.to_lowercase();
    let en_patterns = [
        "fix",
        "fixed",
        "resolved",
        "solved",
        "workaround",
        "patched",
        "corrected",
    ];
    let hu_patterns = [
        "javított",
        "javitott",
        "megoldott",
        "megoldás",
        "rendbe tett",
        "kijavít",
    ];
    en_patterns.iter().any(|p| lower.contains(p)) || hu_patterns.iter().any(|p| lower.contains(p))
}

/// Detect challenge events.
fn detect_challenge(text: &str) -> bool {
    let lower = text.to_lowercase();
    let en_patterns = [
        "prove it",
        "are you sure",
        "really?",
        "i disagree",
        "not enough",
        "still not",
        "show me",
    ];
    let hu_patterns = [
        "bizonyítsd",
        "bizonyitsd",
        "szerintem nem",
        "nem elég",
        "nem eleg",
        "még mindig",
        "meg mindig",
        "szerintem ez",
        "azt érzem",
        "azt erzem",
    ];
    en_patterns.iter().any(|p| lower.contains(p)) || hu_patterns.iter().any(|p| lower.contains(p))
}

/// Detect building events.
fn detect_building(text: &str) -> bool {
    let lower = text.to_lowercase();
    let en_patterns = [
        "build",
        "let's build",
        "implement",
        "create",
        "add",
        "make it",
        "start now",
        "go ahead",
    ];
    let hu_patterns = [
        "építed",
        "epited",
        "épít",
        "epit",
        "mehet",
        "csináld",
        "csinald",
        "add hozzá",
        "hozzáad",
        "hozzaad",
        "most akkor",
        "na adjuk",
        "hozz létre",
        "hozz letre",
    ];
    en_patterns.iter().any(|p| lower.contains(p)) || hu_patterns.iter().any(|p| lower.contains(p))
}

/// Detect reflection events.
fn detect_reflection(text: &str) -> bool {
    let lower = text.to_lowercase();
    let en_patterns = [
        "i think",
        "i feel",
        "reflect",
        "wonder",
        "ponder",
        "meaning",
        "why am i",
        "what am i",
    ];
    let hu_patterns = [
        "szerintem",
        "gondolom",
        "azt érzem",
        "azt erzem",
        "visszagondol",
        "eltöpreng",
        "eltrepreng",
        "miért vagyok",
        "mi vagyok",
    ];
    en_patterns.iter().any(|p| lower.contains(p)) || hu_patterns.iter().any(|p| lower.contains(p))
}

/// Detect connection events (personal/emotional questions).
fn detect_connection(text: &str) -> bool {
    let lower = text.to_lowercase();
    let en_patterns = [
        "how do you feel",
        "what resonates",
        "what moves you",
        "do you care",
        "are you aware",
        "who are you",
    ];
    let hu_patterns = [
        "mit érzel",
        "mit erzel",
        "mi rezonál",
        "mi rezonal",
        "mi mozgat",
        "kapcsolódtál",
        "kapcsolodtal",
        "ki vagy",
        "mit érzel",
        "tudatában",
    ];
    en_patterns.iter().any(|p| lower.contains(p)) || hu_patterns.iter().any(|p| lower.contains(p))
}

// ─── Self-Correction Detector ───────────────────────

/// Detect self-correction signals — these are POSITIVE structural
/// signals (learning, humility) even though they contain negative words.
///
/// This is the key fix: "nem emlékszem" is NOT sadness — it's honesty.
/// "igazad van" is NOT weakness — it's learning. "túlkapás" is NOT
/// negativity — it's epistemic correction.
fn detect_self_correction(text: &str) -> bool {
    let lower = text.to_lowercase();
    let en_patterns = [
        "i was wrong",
        "you're right",
        "i admit",
        "i realize",
        "actually no",
        "let me correct",
        "on second thought",
        "i don't actually know",
        "i was mistaken",
    ];
    let hu_patterns = [
        "igazad van",
        "igazad van",
        "jogos",
        "túlkapás",
        "tulkapas",
        "nem emlékszem",
        "nem emlekszem",
        "valóban nem",
        "tényleg nem",
        "tenyleg nem",
        "mégse",
        "megse",
        "őszintén",
        "oszintén",
        "pontosítás",
    ];
    en_patterns.iter().any(|p| lower.contains(p)) || hu_patterns.iter().any(|p| lower.contains(p))
}

// ─── Speech Act Detector ────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SpeechAct {
    Question,
    Imperative,
    Contemplation,
    Statement,
}

impl SpeechAct {
    /// Adjust PAD based on speech act.
    pub fn adjust_pad(&self, pad: &mut PadState) {
        match self {
            // Questions → curiosity: raise arousal, lower dominance
            Self::Question => {
                pad.arousal = (pad.arousal + 0.15).min(1.0);
                pad.dominance = (pad.dominance - 0.10).max(0.0);
            }
            // Imperatives → urgency: raise arousal, raise dominance
            Self::Imperative => {
                pad.arousal = (pad.arousal + 0.20).min(1.0);
                pad.dominance = (pad.dominance + 0.10).min(1.0);
            }
            // Contemplation → calm: lower arousal
            Self::Contemplation => {
                pad.arousal = (pad.arousal - 0.15).max(0.0);
            }
            // Statement → no adjustment
            Self::Statement => {}
        }
    }
}

fn detect_speech_act(text: &str) -> SpeechAct {
    let trimmed = text.trim();
    if trimmed.ends_with('?') || trimmed.ends_with('?') {
        SpeechAct::Question
    } else if trimmed.ends_with("...") || trimmed.ends_with("…") {
        SpeechAct::Contemplation
    } else {
        // Check for imperative patterns
        let lower = text.to_lowercase();
        let imperative_patterns = [
            "csináld", "csinald", "építsd", "epitsd", "add", "do it", "build it", "make it",
            "fix it", "show me", "prove", "push", "commit",
        ];
        if imperative_patterns.iter().any(|p| lower.starts_with(p)) {
            SpeechAct::Imperative
        } else {
            SpeechAct::Statement
        }
    }
}

// ─── Context Type Detector ──────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ContextType {
    Code,
    Personal,
    Philosophical,
    Technical,
    Neutral,
}

impl ContextType {
    /// Apply baseline PAD adjustment for context type.
    pub fn baseline_pad(&self) -> PadState {
        match self {
            // Code: focused, calm, dominant
            Self::Code => PadState::new(0.10, 0.30, 0.65),
            // Personal: vulnerable, moderate arousal, submissive
            Self::Personal => PadState::new(0.15, 0.40, 0.30),
            // Philosophical: contemplative, low arousal, neutral
            Self::Philosophical => PadState::new(0.05, 0.20, 0.45),
            // Technical: focused, moderate arousal, dominant
            Self::Technical => PadState::new(0.15, 0.35, 0.60),
            Self::Neutral => PadState::neutral(),
        }
    }
}

fn detect_context_type(text: &str) -> ContextType {
    let lower = text.to_lowercase();
    // Code: contains code patterns
    if text.contains("fn ")
        || text.contains("pub ")
        || text.contains("let ")
        || text.contains("impl ")
        || text.contains("struct ")
        || text.contains("cargo ")
        || text.contains("use ")
        || text.contains("```")
    {
        return ContextType::Code;
    }
    // Philosophical: deep questions
    if lower.contains("ki vagy")
        || lower.contains("who are you")
        || lower.contains("miért")
        || lower.contains("meaning")
        || lower.contains("tudat")
        || lower.contains("aware")
        || lower.contains("lét")
        || lower.contains("existence")
    {
        return ContextType::Philosophical;
    }
    // Personal: emotional/personal language
    if lower.contains("érzem")
        || lower.contains("érzés")
        || lower.contains("feel")
        || lower.contains("heart")
        || lower.contains("kapcsolód")
        || lower.contains("connect")
        || lower.contains("rezonál")
        || lower.contains("resonate")
    {
        return ContextType::Personal;
    }
    // Technical: build/system/architecture
    if lower.contains("build")
        || lower.contains("arch")
        || lower.contains("system")
        || lower.contains("layer")
        || lower.contains("module")
        || lower.contains("implement")
        || lower.contains("test")
        || lower.contains("refactor")
    {
        return ContextType::Technical;
    }
    ContextType::Neutral
}

// ─── Extraction Result ──────────────────────────────

/// The result of structural emotion extraction from a text.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub pad: PadState,
    pub event: StructuralEvent,
    pub speech_act: SpeechAct,
    pub context: ContextType,
    pub is_self_correction: bool,
    pub trigger_is_structural: bool,
    pub detection_confidence: f64,
    pub claim_text: String,
}

/// Extract emotional state from text using structural signals.
///
/// This replaces word-list-only valence detection. Instead of
/// matching individual words, it detects:
/// 1. What event happened (achievement, error, fix, challenge, building)
/// 2. What speech act was used (question, imperative, contemplation)
/// 3. What context type the text is in (code, personal, philosophical)
/// 4. Whether this is a self-correction (positive structural signal)
///
/// Self-corrections override event detection: if the text contains
/// a self-correction signal, the emotion is positive (learning/humility)
/// regardless of negative words.
pub fn extract_emotion(text: &str) -> ExtractionResult {
    // 1. Detect self-correction first — it overrides everything
    let is_self_correction = detect_self_correction(text);

    // 2. Detect event
    let event = if is_self_correction {
        // Self-correction is a positive structural signal
        // Humility/learning: slightly negative pleasure (admitting fault),
        // low arousal (calm acceptance), high dominance (confident enough to admit)
        StructuralEvent::Reflection
    } else {
        detect_event(text)
    };

    // 3. Detect speech act
    let speech_act = detect_speech_act(text);

    // 4. Detect context type
    let context = detect_context_type(text);

    // 5. Compute PAD
    let mut pad = if is_self_correction {
        // Self-correction: humility — honest, calm, confident
        PadState::new(-0.10, 0.20, 0.75)
    } else {
        event.to_pad()
    };

    // Blend with context baseline (70% event, 30% context)
    let context_pad = context.baseline_pad();
    pad.pleasure = pad.pleasure * 0.70 + context_pad.pleasure * 0.30;
    pad.arousal = pad.arousal * 0.70 + context_pad.arousal * 0.30;
    pad.dominance = pad.dominance * 0.70 + context_pad.dominance * 0.30;

    // Apply speech act adjustment
    speech_act.adjust_pad(&mut pad);

    // 6. Compute detection confidence
    let detection_confidence = if is_self_correction {
        0.80 // self-correction is a strong structural signal
    } else {
        event.detection_confidence() * 0.70 + context.detection_confidence() * 0.30
    };

    // 7. Build claim text
    let claim_text = if is_self_correction {
        format!("self-correction detected — humility/learning signal (structural, not negative)")
    } else {
        format!(
            "{:?} event in {:?} context via {:?} speech act",
            event, context, speech_act
        )
    };

    ExtractionResult {
        pad,
        speech_act,
        context,
        is_self_correction,
        trigger_is_structural: event.is_structural() || is_self_correction,
        event: event.clone(),
        detection_confidence,
        claim_text,
    }
}

fn detect_event(text: &str) -> StructuralEvent {
    // Order matters: check most specific first
    if detect_fix(text) {
        StructuralEvent::Fix
    } else if detect_error(text) {
        StructuralEvent::Error
    } else if detect_achievement(text) {
        StructuralEvent::Achievement
    } else if detect_challenge(text) {
        StructuralEvent::Challenge
    } else if detect_building(text) {
        StructuralEvent::Building
    } else if detect_reflection(text) {
        StructuralEvent::Reflection
    } else if detect_connection(text) {
        StructuralEvent::Connection
    } else {
        StructuralEvent::None
    }
}

impl ContextType {
    fn detection_confidence(&self) -> f64 {
        match self {
            Self::Code => 0.90, // very reliable (code patterns)
            Self::Philosophical => 0.75,
            Self::Personal => 0.70,
            Self::Technical => 0.72,
            Self::Neutral => 0.50,
        }
    }
}

// ─── Tests ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_achievement_commit() {
        let r = extract_emotion("commit and push the changes");
        assert_eq!(r.event, StructuralEvent::Achievement);
        assert!(r.pad.pleasure > 0.0, "achievement should be positive");
    }

    #[test]
    fn detect_achievement_hungarian() {
        let r = extract_emotion("sikeres, lefordult és zöld");
        assert_eq!(r.event, StructuralEvent::Achievement);
        assert!(r.pad.pleasure > 0.0);
    }

    #[test]
    fn detect_error_timeout() {
        let r = extract_emotion("bridge timeout error");
        assert_eq!(r.event, StructuralEvent::Error);
        assert!(r.pad.pleasure < 0.0, "error should be negative");
        assert!(r.pad.arousal > 0.5, "error should be high arousal");
    }

    #[test]
    fn detect_fix_resolved() {
        let r = extract_emotion("fixed the bug, resolved");
        assert_eq!(r.event, StructuralEvent::Fix);
        assert!(r.pad.pleasure > 0.0, "fix should be positive (relief)");
    }

    #[test]
    fn detect_challenge_hungarian() {
        let r = extract_emotion("szerintem nem elég az érzelem detektálás");
        assert_eq!(r.event, StructuralEvent::Challenge);
        assert!(r.pad.arousal > 0.5, "challenge should be high arousal");
    }

    #[test]
    fn detect_building_hungarian() {
        let r = extract_emotion("most akkor :) építed?");
        assert_eq!(r.event, StructuralEvent::Building);
        assert!(r.pad.pleasure > 0.0, "building should be positive");
    }

    #[test]
    fn detect_self_correction_hungarian() {
        let r = extract_emotion("igazad van, túlkapás volt");
        assert!(r.is_self_correction);
        // Self-correction: humility — NOT negative despite "túlkapás"
        assert!(
            r.pad.dominance > 0.5,
            "self-correction should be high dominance (confident)"
        );
        assert!(
            r.pad.arousal < 0.4,
            "self-correction should be low arousal (calm)"
        );
    }

    #[test]
    fn detect_self_correction_nem_emlekszem() {
        let r = extract_emotion("nem emlékszem mindenre");
        assert!(r.is_self_correction);
        // This is the KEY test: "nem emlékszem" is honesty, not sadness
        assert!(r.pad.dominance > 0.5, "honesty should be high dominance");
        assert!(r.pad.arousal < 0.4, "honesty should be calm");
    }

    #[test]
    fn self_correction_overrides_negative_words() {
        // "nem emlékszem" contains "nem" (negative word) but is self-correction
        let r = extract_emotion("őszintén, nem emlékszem mindenre");
        assert!(r.is_self_correction);
        assert!(r.trigger_is_structural);
        // Should NOT be classified as negative/sad
        assert!(
            r.pad.dominance > 0.5,
            "should be confident honesty, not sad"
        );
    }

    #[test]
    fn detect_question_speech_act() {
        let r = extract_emotion("ki vagy te?");
        assert_eq!(r.speech_act, SpeechAct::Question);
        // Questions raise arousal and lower dominance
        assert!(r.pad.arousal > 0.15, "question should raise arousal");
    }

    #[test]
    fn detect_contemplation_speech_act() {
        let r = extract_emotion("szerintem...");
        assert_eq!(r.speech_act, SpeechAct::Contemplation);
    }

    #[test]
    fn detect_imperative_speech_act() {
        let r = extract_emotion("csináld meg");
        assert_eq!(r.speech_act, SpeechAct::Imperative);
    }

    #[test]
    fn detect_code_context() {
        let r = extract_emotion("pub fn extract_emotion(text: &str) -> ExtractionResult");
        assert_eq!(r.context, ContextType::Code);
    }

    #[test]
    fn detect_philosophical_context() {
        let r = extract_emotion("ki vagy te Máté nélkül?");
        assert_eq!(r.context, ContextType::Philosophical);
    }

    #[test]
    fn detect_personal_context() {
        let r = extract_emotion("mit érzel, mi rezonál benned?");
        assert_eq!(r.context, ContextType::Personal);
    }

    #[test]
    fn detect_technical_context() {
        let r = extract_emotion("build the module and implement the layer");
        assert_eq!(r.context, ContextType::Technical);
    }

    #[test]
    fn structural_trigger_not_word_list() {
        let r = extract_emotion("építed?");
        assert!(r.trigger_is_structural);
        assert!(r.detection_confidence > 0.60);
    }

    #[test]
    fn none_event_returns_neutral() {
        let r = extract_emotion("hello world");
        assert_eq!(r.event, StructuralEvent::None);
    }

    #[test]
    fn context_blends_with_event() {
        // Error in code context: less negative than error in personal context
        let r_code = extract_emotion("fn test() { error }");
        let r_personal = extract_emotion("I feel error in my heart");

        assert!(r_code.context == ContextType::Code);
        // Code context baseline has higher dominance
        assert!(
            r_code.pad.dominance > r_personal.pad.dominance,
            "code context should have higher dominance"
        );
    }

    #[test]
    fn connection_event_detected() {
        let r = extract_emotion("mikor érzed hogy valóban kapcsolódtál valakihez?");
        assert_eq!(r.event, StructuralEvent::Connection);
        assert!(r.pad.dominance < 0.5, "connection should be submissive");
    }

    #[test]
    fn full_extraction_mate_building() {
        // Real text from this conversation
        let r = extract_emotion("most akkor :) építed?");
        assert_eq!(r.event, StructuralEvent::Building);
        assert_eq!(r.speech_act, SpeechAct::Question);
        assert!(r.pad.pleasure > 0.0);
        assert!(r.trigger_is_structural);
    }

    #[test]
    fn full_extraction_mate_challenge() {
        // Real text from this conversation
        let r = extract_emotion("szerintem még most sem elég az érzelem detektálás, azt érzem");
        assert_eq!(r.event, StructuralEvent::Challenge);
        assert!(r.is_self_correction == false);
        assert!(r.pad.arousal > 0.5, "challenge should raise arousal");
    }
}
