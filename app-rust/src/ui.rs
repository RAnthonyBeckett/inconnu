use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use grapevine_workbench::{
    diagnostics,
    model::{BuildKind, Edit, Language, Preview, RecordView, Role, SheetEdit, Summary},
    store::Library,
};
use rfd::FileDialog;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkArea {
    Player,
    Storyteller,
    Builder,
    BuildTest,
}

impl WorkArea {
    fn role(self) -> Option<Role> {
        match self {
            Self::Player => Some(Role::Player),
            Self::Storyteller | Self::Builder | Self::BuildTest => Some(Role::Storyteller),
        }
    }
}

#[derive(Clone, Debug)]
enum PendingAction {
    RumorAudience(String),
    BoonEntry(String, Option<String>),
    SearchRecord(Option<String>),
    PlayerProfile(Option<String>),
    AssociationRecord(Option<String>),
    StoryRecord(grapevine_workbench::story_work::Kind, Option<String>),
    PlotRecord(Option<String>),
    WorldRecord(Option<String>, String),
    NewSession,
    LoadSession(grapevine_workbench::game_sessions::SessionRequest),
    SelectPoints(String),
    NewLibrary,
    OpenLibrary,
    OpenRecent(PathBuf),
    Refresh,
    SelectRecord(String),
    CommitImport,
    Close,
    CreateBuild(BuildKind),
    Route(WorkArea),
    CreateCharacter,
    OpenXp(String),
    RefreshXp,
    RosterXp,
    SaveMenu(Box<grapevine_workbench::menus::MenuPreview>),
    SelectMenu(Option<String>),
}

#[derive(Clone, Copy, Debug, Default)]
struct SmokeControls {
    storyteller: Option<egui::Rect>,
    builder: Option<egui::Rect>,
    build_test: Option<egui::Rect>,
    player: Option<egui::Rect>,
    portuguese: Option<egui::Rect>,
    name: Option<egui::Rect>,
    save: Option<egui::Rect>,
    commit_import: Option<egui::Rect>,
    create_build: Option<egui::Rect>,
    english: Option<egui::Rect>,
    xp: Option<egui::Rect>,
    menu_search: Option<egui::Rect>,
    menu_close: Option<egui::Rect>,
    menu_save: Option<egui::Rect>,
    scenario: Option<egui::Rect>,
    purchase: Option<egui::Rect>,
    playtime: Option<egui::Rect>,
    print: Option<egui::Rect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SmokePhase {
    InitialPlayerFrame,
    RecentMenu,
    RecentOpen,
    RecentOpened,
    ClickStoryteller,
    ClickImportCommit,
    ClickName,
    TypeName,
    ScrollClan,
    TypeClan,
    CheckClan,
    ClickPortuguese,
    ClickSave,
    ClickBuilder,
    MenuType,
    MenuInput,
    MenuCheck,
    MenuSave,
    MenuSaved,
    MenuClose,
    MenuResume,
    ClickBuildTest,
    XpOpen,
    XpInitialize,
    XpReady,
    XpAmount,
    XpPost,
    XpAwardSaved,
    XpNegativeAmount,
    XpNegativePost,
    XpRejected,
    XpCloseAfterCapture,
    PlaytimeClick,
    PlaytimeOpen,
    PlaytimeLoadAwards,
    PlaytimePending,
    PlaytimeClassified,
    PlaytimeSeparate,
    PlaytimeCalculated,
    PlaytimeUnqualified,
    PlaytimeCloseAfterCapture,
    PlaytimeSavePrivate,
    PlaytimePrivateSaved,
    PlaytimePrivateConfirm,
    PlaytimePrivateLoaded,
    PlaytimePrivateDone,
    ScenarioClick,
    ScenarioOpen,
    ScenarioProfile,
    ScenarioPower0,
    ScenarioIn0,
    ScenarioPower1,
    ScenarioOut1,
    ScenarioAssumed,
    ScenarioBudgetClick,
    ScenarioBudgetType,
    ScenarioCheck,
    ScenarioKeep,
    ScenarioFamilyClick,
    ScenarioFamilyMenu,
    ScenarioFamilyChoice,
    ScenarioFamilyChecked,
    ScenarioTierClick,
    ScenarioTierMenu,
    ScenarioTierChecked,
    ScenarioFinish,
    PurchaseClick,
    PurchaseOpen,
    ReviewSaved,
    ReviewReused,
    PurchaseQuoted,
    PurchaseCommitted,
    PurchaseDone,
    PrintClick,
    PrintOpen,
    PrintMechanicsEnabled,
    PrintReady,
    PrintCloseAfterCapture,
    CancelSelect,
    CancelOpen,
    CancelQuoted,
    CancelCommitted,
    CancelDone,
    BatchAddFirst,
    BatchFirstAdded,
    BatchAddSecond,
    BatchSecondAdded,
    BatchApply,
    BatchApplied,
    BatchDone,
    AdvancementQueue,
    AdvancementSave,
    AdvancementSaved,
    AdvancementReview,
    AdvancementApplied,
    PublishedClick,
    PublishedOpen,
    PublishedPrepare,
    PublishedQuoted,
    PublishedSaved,
    OrdinaryClick,
    OrdinaryOpen,
    OrdinaryPrepare,
    OrdinaryQuoted,
    OrdinarySaved,
    CorrectionSelect,
    CorrectionOpen,
    CorrectionPrepare,
    CorrectionQuoted,
    CorrectionSaved,
    CreationOpen,
    CreationSourceChoose,
    CreationSourceChosen,
    CreationPrepare,
    CreationQuoted,
    CreationSaved,
    MetOpen,
    MetPrepare,
    MetQuoted,
    MetSaved,
    LifecycleOpen,
    LifecyclePrepare,
    LifecycleQuoted,
    LifecycleSaved,
    MixedSetup,
    MixedPreview,
    MixedSaved,
    MixedApply,
    MixedApplied,
    LegacyProfileSetup,
    LegacyProfilePreview,
    LegacyProfileApply,
    LegacyProfileApplied,
    LegacyCalendarSetup,
    LegacyCalendarPreview,
    LegacyCalendarApply,
    LegacyCalendarApplied,
    LegacyCalendarLoad,
    LegacyCalendarLoaded,
    SessionClick,
    SessionOpen,
    SessionPrepared,
    SessionScheduled,
    SessionPreviewReady,
    SessionPreviewed,
    SessionCommitted,
    SessionDone,
    PointsClick,
    PointsOpen,
    PointsCreatePrepared,
    PointsCreated,
    PointsOpeningPrepared,
    PointsOpened,
    PointsAwardPrepared,
    PointsAwarded,
    PointsDone,
    WorldPrepare,
    WorldSave,
    WorldCheck,
    WorldDone,
    PlotPrepare,
    PlotSave,
    PlotCheck,
    PlotDone,
    StoryPrepare(grapevine_workbench::story_work::Kind),
    StorySave(grapevine_workbench::story_work::Kind),
    StoryCheck(grapevine_workbench::story_work::Kind),
    StoryDone(grapevine_workbench::story_work::Kind),
    AssociationPrepare,
    AssociationSave,
    AssociationCheck,
    AssociationDone,
    OperationsPrepare,
    OperationsClick,
    OperationsCheck,
    OperationsDone,
    HistoryPrepare(bool),
    HistoryClick(bool),
    HistoryCheck(bool),
    HistoryDone(bool),
    ProfilePrepare,
    ProfileClick,
    ProfileCheck,
    ProfileDone,
    SearchPrepare,
    SearchSave,
    SearchSaved,
    SearchRun,
    SearchCheck,
    SearchDone,
    TraitPrepare(bool),
    TraitRun(bool),
    TraitCheck(bool),
    TraitDone(bool),
    BoonPrepare,
    BoonSave,
    BoonCheck,
    BoonDone,
    StatusPrepare,
    StatusClick,
    StatusCheck,
    StatusDone,
    ActionLinkPrepare,
    ActionTimelineClick,
    ActionTimelineCheck,
    ActionCopyClick,
    ActionCopyCheck,
    ActionCopyApply,
    ActionCopyApplied,
    ActionCopySave,
    ActionCopySaved,
    ActionLinkDone,
    ChroniclePrepare,
    ChronicleSave,
    ChronicleSaved,
    ChronicleHandover,
    ChronicleChecked,
    ChronicleDone,
    RumorAudiencePrepare,
    RumorAudienceQueue,
    RumorAudienceQueueCheck,
    RumorAudiencePreview,
    RumorAudiencePreviewCheck,
    RumorAudienceSave,
    RumorAudienceCheck,
    RumorAudienceDone,
    DistributionPrepare(bool),
    DistributionRun(bool),
    DistributionCheck(bool),
    DistributionDone(bool),
    SupportPrepare,
    SupportPreview,
    SupportDefaultCheck,
    SupportToggleTraces,
    SupportToggleCheck,
    SupportTracePreview,
    SupportTraceCheck,
    SupportDone,
    CombinedToggle,
    CombinedToggleCheck,
    CombinedPrepare,
    CombinedCheck,
    CombinedDone,
    BuilderAfterXp,

    CreateDraft,
    ScrollTrait,
    TypeTrait,
    CheckTrait,
    ScrollSaveDraft,
    SaveDraft,
    CreateTest,
    CheckTest,
    ClickPlayer,
    Capture,
    AwaitCapture,
}

struct SmokeState {
    output_dir: PathBuf,
    library_path: PathBuf,
    phase: SmokePhase,
    frames: usize,
    started: Instant,
    phase_started: Instant,
    logged_phase: Option<SmokePhase>,
    next_step: Instant,
    failure: Option<String>,
    checks: serde_json::Map<String, serde_json::Value>,
    screenshot_state: Arc<Mutex<Option<std::result::Result<(), String>>>>,
    scenario_before: Option<(i64, String)>,
    batch_previous_selection: Option<String>,
}

pub struct InconnuApp {
    published: published_window::PublishedWindow,
    ordinary: trait_advancement_window::TraitWindow,
    creation: creation_window::CreationWindow,
    met_reference: met_reference_window::MetWindow,
    lifecycle: character_lifecycle_window::LifecycleWindow,
    mixed: mixed_advancement_window::MixedWindow,
    legacy_profile: legacy_profile_window::LegacyWindow,
    legacy_calendar: legacy_calendar_window::CalendarWindow,
    advancement: advancement_window::AdvancementWindow,
    support: support_window::SupportWindow,
    rumor_audience: rumor_audience_window::RumorAudienceWindow,
    world: world_window::WorldState,
    plot: plot_window::PlotWindow,
    story: story_window::StoryWindow,
    associations: association_window::AssociationWindow,
    operations: operations_window::OperationsWindow,
    profiles: profile_window::ProfileWindow,
    chronicle: chronicle_window::ChronicleWindow,
    boons: boon_window::BoonWindow,
    searches: search_window::SearchWindow,
    points_open: bool,
    points_target: Option<String>,
    points_form: crate::points_ui::Form,
    points_ledger: Option<grapevine_workbench::player_points::Ledger>,
    points_revision: Option<i64>,
    points_saved_entry: serde_json::Value,
    session_open: bool,
    session_form: crate::session_ui::Form,
    print_open: bool,
    print_target: Option<String>,
    print_form: crate::print_ui::Form,
    playtime_open: bool,
    playtime_target: Option<String>,
    playtime_form: crate::playtime_ui::Form,
    library: Option<Library>,
    recent_libraries: grapevine_workbench::recent_libraries::RecentLibraries,
    route: WorkArea,
    language: Language,
    summaries: Vec<Summary>,
    selected_id: Option<String>,
    selected_record: Option<RecordView>,
    query: String,
    player_filter: Option<String>,
    edit: Edit,
    saved_edit: Edit,
    sheet_edit: Option<SheetEdit>,
    saved_sheet: Option<SheetEdit>,
    build_source_id: Option<String>,
    revision: Option<i64>,
    import_encoding: String,
    import_preview: Option<Preview>,
    menu_preview: Option<grapevine_workbench::menus::MenuPreview>,
    menu_query: String,
    menu_sources: Vec<grapevine_workbench::menus::MenuSummary>,
    active_menu_hash: Option<String>,
    scenario_open: bool,
    scenario_form: crate::scenario_ui::ScenarioForm,
    scenario_baseline: Option<RecordView>,
    scenario_balance: Option<String>,
    scenario_revision: Option<i64>,
    purchase_open: bool,
    purchase_target: Option<String>,
    purchase_form: crate::purchase_ui::PurchaseForm,
    skip_import_conflicts: bool,
    restore_candidate: Option<PathBuf>,
    pending_action: Option<PendingAction>,
    status: Option<String>,
    error: Option<String>,
    smoke_controls: SmokeControls,
    smoke: Option<SmokeState>,
    pending_raw_events: Vec<egui::Event>,
    create_character_open: bool,
    new_character_kind: String,
    new_character_name: String,
    close_after_confirmation: bool,
    xp_window_open: bool,
    xp_view: Option<grapevine_workbench::xp::XpView>,
    xp_form: crate::xp_ui::XpForm,
    xp_saved_form: crate::xp_ui::XpForm,
    xp_revision: Option<i64>,
    xp_message: Option<String>,
    xp_roster: Option<Vec<grapevine_workbench::xp::XpView>>,
}

impl InconnuApp {
    pub fn new(initial_library: Option<PathBuf>) -> Self {
        let mut app = Self {
            support: Default::default(),
            published: Default::default(),
            ordinary: Default::default(),
            creation: Default::default(),
            met_reference: Default::default(),
            lifecycle: Default::default(),
            mixed: Default::default(),
            legacy_profile: Default::default(),
            legacy_calendar: Default::default(),
            advancement: Default::default(),
            rumor_audience: Default::default(),
            world: Default::default(),
            plot: Default::default(),
            story: Default::default(),
            associations: Default::default(),
            operations: Default::default(),
            profiles: Default::default(),
            chronicle: Default::default(),
            boons: Default::default(),
            searches: Default::default(),
            points_open: false,
            points_target: None,
            points_form: Default::default(),
            points_ledger: None,
            points_revision: None,
            points_saved_entry: serde_json::Value::Null,
            session_open: false,
            session_form: Default::default(),
            library: None,
            recent_libraries: Default::default(),
            route: WorkArea::Player,
            language: Language::English,
            summaries: Vec::new(),
            selected_id: None,
            selected_record: None,
            query: String::new(),
            player_filter: None,
            edit: Edit::default(),
            saved_edit: Edit::default(),
            sheet_edit: None,
            saved_sheet: None,
            build_source_id: None,
            revision: None,
            import_encoding: "windows-1252".to_owned(),
            import_preview: None,
            menu_preview: None,
            menu_query: String::new(),
            menu_sources: Vec::new(),
            active_menu_hash: None,
            scenario_open: false,
            scenario_form: Default::default(),
            scenario_baseline: None,
            scenario_balance: None,
            scenario_revision: None,
            purchase_open: false,
            purchase_target: None,
            purchase_form: Default::default(),
            print_open: false,
            print_target: None,
            print_form: Default::default(),
            playtime_open: false,
            playtime_target: None,
            playtime_form: Default::default(),
            skip_import_conflicts: false,
            restore_candidate: None,
            pending_action: None,
            status: None,
            error: None,
            smoke_controls: SmokeControls::default(),
            smoke: None,
            pending_raw_events: Vec::new(),
            create_character_open: false,
            new_character_kind: "Vampire".to_owned(),
            new_character_name: String::new(),
            close_after_confirmation: false,
            xp_window_open: false,
            xp_view: None,
            xp_form: Default::default(),
            xp_saved_form: Default::default(),
            xp_revision: None,
            xp_message: None,
            xp_roster: None,
        };
        if let Some(path) = initial_library {
            app.open_path(path);
        }
        app
    }

    fn t<'a>(&self, english: &'a str, portuguese: &'a str) -> &'a str {
        match self.language {
            Language::English => english,
            Language::Portuguese => portuguese,
        }
    }

    fn has_unsaved_edit(&self) -> bool {
        (self.rumor_audience.dirty()
            || self.world.dirty()
            || self.plot.dirty()
            || self.story.dirty()
            || self.associations.dirty()
            || self.profiles.dirty()
            || self.chronicle.dirty()
            || self.boons.dirty()
            || self.searches.dirty()
            || self.points_form.dirty
            || self.session_form.dirty
            || self.session_form.quote.is_some()
            || self.purchase_form.dirty
            || self.published.dirty
            || self.met_reference.dirty
            || self.lifecycle.dirty
            || self.mixed.dirty
            || self.legacy_profile.dirty
            || self.legacy_calendar.dirty
            || self.creation.dirty
            || self.ordinary.dirty
            || self.purchase_form.quote.is_some())
            || self.library.is_some()
                && (self.edit != self.saved_edit
                    || self.sheet_edit != self.saved_sheet
                    || (self.xp_form.is_dirty() && self.xp_form != self.xp_saved_form))
    }

    fn active_role(&self) -> Option<Role> {
        self.route.role()
    }

    fn log_error(&mut self, operation: &str, error: &grapevine_workbench::model::AppError) {
        diagnostics::record(operation, error.code);
        let prefix = self.t("The operation failed", "A operação falhou");
        self.error = Some(format!("{prefix} ({}): {}", error.code, error.message));
        self.status = None;
    }

    fn set_library(&mut self, library: Library) {
        match library.revision() {
            Ok(revision) => {
                self.clear_purchases();
                self.clear_scenarios();
                self.library = Some(library);
                self.revision = Some(revision);
                self.summaries.clear();
                self.selected_id = None;
                self.selected_record = None;
                self.edit = Edit::default();
                self.saved_edit = Edit::default();
                self.sheet_edit = None;
                self.saved_sheet = None;
                self.build_source_id = None;
                self.xp_window_open = false;
                self.xp_view = None;
                self.xp_form = Default::default();
                self.xp_saved_form = Default::default();
                self.xp_revision = None;
                self.xp_message = None;
                self.xp_roster = None;
                self.import_preview = None;
                self.menu_preview = None;
                self.menu_query.clear();
                self.player_filter = None;
                self.status = Some(self.t("Library opened", "Biblioteca aberta").to_owned());
                self.error = None;
                self.refresh_summaries(false);
                self.refresh_menu_sources();
                if let Some(library) = &self.library {
                    self.recent_libraries.remember(&library.path);
                }
            }
            Err(error) => self.log_error("library.revision", &error),
        }
    }

    fn create_library_dialog(&mut self) {
        let path = FileDialog::new()
            .set_title(self.t(
                "Create a local Inconnu library",
                "Criar uma biblioteca local do Inconnu",
            ))
            .set_file_name("Inconnu.gvlib")
            .add_filter("Inconnu library", &["gvlib", "sqlite", "db"])
            .save_file();
        if let Some(path) = path {
            let title = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .filter(|stem| !stem.trim().is_empty())
                .unwrap_or("Inconnu Library")
                .to_owned();
            match Library::create(&path, &title) {
                Ok(library) => self.set_library(library),
                Err(error) => self.log_error("library.create", &error),
            }
        }
    }

    fn open_library_dialog(&mut self) {
        let path = FileDialog::new()
            .set_title(self.t(
                "Open a local Inconnu library",
                "Abrir uma biblioteca local do Inconnu",
            ))
            .add_filter("Inconnu library", &["gvlib", "sqlite", "db"])
            .pick_file();
        if let Some(path) = path {
            self.open_path(path);
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        match Library::open(&path) {
            Ok(library) => self.set_library(library),
            Err(error) => self.log_error("library.open", &error),
        }
    }

    fn refresh_summaries(&mut self, preserve_selection: bool) {
        self.clear_scenarios();
        let Some(role) = self.active_role() else {
            return;
        };
        let Some(library) = self.library.as_ref() else {
            self.summaries.clear();
            self.selected_record = None;
            return;
        };
        match library.summaries(role) {
            Ok(mut summaries) => {
                summaries.retain(|r| match self.route {
                    WorkArea::Builder => r.build == Some(BuildKind::Draft),
                    WorkArea::BuildTest => r.build == Some(BuildKind::Test),
                    _ => r.build.is_none(),
                });
                let old_selection = preserve_selection
                    .then(|| self.selected_id.clone())
                    .flatten();
                self.summaries = summaries;
                let keep = old_selection
                    .filter(|id| self.summaries.iter().any(|summary| &summary.id == id));
                self.selected_id = keep.or_else(|| {
                    self.summaries
                        .iter()
                        .find(|item| grapevine_workbench::sheet::is_character_kind(&item.kind))
                        .or_else(|| self.summaries.first())
                        .map(|item| item.id.clone())
                });
                if let Some(id) = self.selected_id.clone() {
                    let keep_draft = self.has_unsaved_edit();
                    self.load_record(&id, keep_draft);
                } else {
                    self.selected_record = None;
                    self.edit = Edit::default();
                    self.saved_edit = Edit::default();
                    self.sheet_edit = None;
                    self.saved_sheet = None;
                }
                self.error = None;
            }
            Err(error) => self.log_error("library.summaries", &error),
        }
    }

    fn load_record(&mut self, id: &str, preserve_draft: bool) {
        let Some(role) = self.active_role() else {
            self.selected_record = None;
            return;
        };
        let Some(library) = self.library.as_ref() else {
            self.selected_record = None;
            return;
        };
        match library.record(id, role) {
            Ok(record) => {
                self.selected_record = Some(record.clone());
                if role == Role::Storyteller && !preserve_draft {
                    self.edit = Edit {
                        name: record.name.clone(),
                        status: record.status.clone(),
                        notes: record.notes.clone(),
                    };
                    self.saved_edit = self.edit.clone();
                    self.sheet_edit = library.sheet_edit(id, Role::Storyteller).ok();
                    self.saved_sheet = self.sheet_edit.clone();
                }
                self.error = None;
            }
            Err(error) => self.log_error("library.record", &error),
        }
    }

    fn select_record(&mut self, id: String) {
        let searches = std::mem::take(&mut self.searches);
        self.clear_purchases();
        self.searches = searches;
        self.clear_scenarios();
        self.selected_id = Some(id.clone());
        self.query.clear();
        self.load_record(&id, false);
    }

    fn change_route(&mut self, route: WorkArea) {
        if self.route == route {
            return;
        }
        self.clear_purchases();
        self.clear_scenarios();
        self.menu_preview = None;
        self.menu_query.clear();
        self.route = route;
        self.refresh_menu_sources();
        if route == WorkArea::Player {
            self.xp_window_open = false;
            self.xp_view = None;
            self.xp_roster = None;
            self.xp_form = Default::default();
            self.xp_saved_form = Default::default();
            self.xp_revision = None;
            self.xp_message = None;
        }
        if route.role().is_some() {
            self.refresh_summaries(true);
        } else {
            self.selected_record = None;
        }
    }

    fn save_edit(&mut self) {
        let (Some(library), Some(id), Some(revision)) = (
            self.library.as_mut(),
            self.selected_id.clone(),
            self.revision,
        ) else {
            return;
        };
        if self.route == WorkArea::Player {
            return;
        }
        let result = if let Some(sheet) = &self.sheet_edit {
            library.save_character(&id, &self.edit, sheet, revision, Role::Storyteller)
        } else {
            library.save_edit(&id, &self.edit, revision, Role::Storyteller)
        };
        match result {
            Ok(revision) => {
                self.revision = Some(revision);
                self.saved_edit = self.edit.clone();
                self.saved_sheet = self.sheet_edit.clone();
                if let Some(record) = self.selected_record.as_mut() {
                    record.name = self.edit.name.clone();
                    record.status = self.edit.status.clone();
                    record.notes = self.edit.notes.clone();
                }
                if let Some(summary) = self.summaries.iter_mut().find(|item| item.id == id) {
                    summary.name = self.edit.name.clone();
                }
                self.status = Some(self.t("Changes saved", "Alterações salvas").to_owned());
                self.error = None;
                self.load_record(&id, false);
            }
            Err(error) => self.log_error("record.save_edit", &error),
        }
    }

    fn choose_import_file(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let path = FileDialog::new()
            .set_title(self.t(
                "Preview a game, exchange or menu file",
                "Pré-visualizar jogo, intercâmbio ou menu",
            ))
            .add_filter("Grapevine source", &["gex", "gv3", "gvm", "xml"])
            .add_filter("All files", &["*"])
            .pick_file();
        if let Some(path) = path {
            self.import_preview = None;
            self.menu_preview = None;
            self.menu_query.clear();
            match grapevine_workbench::menus::read_with_encoding(&path, &self.import_encoding) {
                Ok(Some(menu)) => {
                    self.menu_preview = Some(menu);
                    self.menu_query.clear();
                    self.import_preview = None;
                    self.error = None;
                    return;
                }
                Ok(None) => {}
                Err(error) => {
                    self.log_error("menu.preview", &error);
                    return;
                }
            }
            let Some(library) = self.library.as_ref() else {
                return;
            };
            match library.preview_import(&path, &self.import_encoding, Role::Storyteller) {
                Ok(preview) => {
                    self.import_preview = Some(preview);
                    self.skip_import_conflicts = false;
                    self.error = None;
                    self.status = Some(
                        self.t(
                            "Import preview is ready. Review it before committing.",
                            "A pré-visualização está pronta. Revise antes de confirmar.",
                        )
                        .to_owned(),
                    );
                }
                Err(error) => self.log_error("import.preview", &error),
            }
        }
    }

    fn commit_import(&mut self) {
        let (Some(library), Some(revision), Some(preview)) = (
            self.library.as_mut(),
            self.revision,
            self.import_preview.as_ref(),
        ) else {
            return;
        };
        let skip_conflicts = self.skip_import_conflicts;
        match library.commit_import(preview, revision, skip_conflicts, Role::Storyteller) {
            Ok(result) => {
                self.revision = Some(result.revision);
                self.import_preview = None;
                self.status = Some(format!(
                    "{}: {} · {}: {}",
                    self.t("Imported", "Importados"),
                    result.inserted,
                    self.t("Skipped", "Ignorados"),
                    result.skipped
                ));
                self.error = None;
                self.refresh_summaries(true);
            }
            Err(error) => self.log_error("import.commit", &error),
        }
    }

    fn backup_library(&mut self) {
        let Some(library) = self.library.as_ref() else {
            return;
        };
        if self.route != WorkArea::Storyteller {
            return;
        }
        let path = FileDialog::new()
            .set_title(self.t(
                "Save a library backup",
                "Salvar uma cópia de segurança da biblioteca",
            ))
            .set_file_name("Inconnu-backup.gvbackup")
            .add_filter("Inconnu backup", &["gvbackup", "db", "sqlite"])
            .save_file();
        if let Some(path) = path {
            match library.backup(&path, Role::Storyteller) {
                Ok(()) => {
                    self.status = Some(
                        self.t("Backup saved", "Cópia de segurança salva")
                            .to_owned(),
                    );
                    self.error = None;
                }
                Err(error) => self.log_error("library.backup", &error),
            }
        }
    }

    fn restore_library_dialog(&mut self) {
        if self.library.is_none() || self.route != WorkArea::Storyteller {
            return;
        }
        let path = FileDialog::new()
            .set_title(self.t(
                "Choose a Inconnu backup to restore",
                "Escolher uma cópia do Inconnu para restaurar",
            ))
            .add_filter("Inconnu backup", &["gvbackup", "db", "sqlite"])
            .pick_file();
        if let Some(path) = path {
            self.restore_candidate = Some(path);
        }
    }

    fn restore_library(&mut self) {
        let (Some(library), Some(path), Some(revision)) = (
            self.library.as_mut(),
            self.restore_candidate.clone(),
            self.revision,
        ) else {
            return;
        };
        match library.restore(&path, revision, Role::Storyteller) {
            Ok(revision) => {
                self.revision = Some(revision);
                self.restore_candidate = None;
                self.status = Some(
                    self.t("Backup restored", "Cópia de segurança restaurada")
                        .to_owned(),
                );
                self.error = None;
                self.refresh_summaries(false);
            }
            Err(error) => self.log_error("library.restore", &error),
        }
    }

    fn reload_library(&mut self) {
        let Some(library) = self.library.as_ref() else {
            return;
        };
        match library.revision() {
            Ok(revision) => {
                self.revision = Some(revision);
                self.status = Some(
                    self.t("Library refreshed", "Biblioteca atualizada")
                        .to_owned(),
                );
                self.error = None;
                self.refresh_summaries(false);
            }
            Err(error) => self.log_error("library.refresh", &error),
        }
    }

    fn create_character(&mut self) {
        let (Some(library), Some(revision)) = (self.library.as_mut(), self.revision) else {
            return;
        };
        match library.create_character(
            &self.new_character_kind,
            &self.new_character_name,
            revision,
            Role::Storyteller,
        ) {
            Ok(id) => {
                self.revision = library.revision().ok();
                self.selected_id = Some(id);
                self.create_character_open = false;
                self.new_character_name.clear();
                self.refresh_summaries(true);
                self.status = Some(self.t("Character created", "Personagem criado").to_owned());
                self.error = None;
            }
            Err(error) => self.log_error("record.create_character", &error),
        }
    }

    fn request_action(&mut self, action: PendingAction) {
        if self.has_unsaved_edit() {
            self.pending_action = Some(action);
        } else {
            self.perform_action(action);
        }
    }

    fn perform_action(&mut self, action: PendingAction) {
        match action {
            PendingAction::RumorAudience(id) => self.load_rumor_audience(id),
            PendingAction::BoonEntry(holder, entry) => self.load_boon_entry(holder, entry),
            PendingAction::SearchRecord(id) => self.load_search(id),
            PendingAction::PlayerProfile(id) => self.load_profile(id),
            PendingAction::AssociationRecord(id) => self.load_associations(id),
            PendingAction::StoryRecord(kind, id) => self.load_story(kind, id),
            PendingAction::PlotRecord(id) => self.load_plot(id),
            PendingAction::WorldRecord(id, kind) => self.load_world(id, kind),
            PendingAction::NewSession => self.new_session(),
            PendingAction::LoadSession(request) => self.load_scheduled_session(request),
            PendingAction::SelectPoints(id) => self.select_points(id),
            PendingAction::NewLibrary => self.create_library_dialog(),
            PendingAction::OpenLibrary => self.open_library_dialog(),
            PendingAction::OpenRecent(path) => self.open_path(path),
            PendingAction::Refresh => self.reload_library(),
            PendingAction::SelectRecord(id) => self.select_record(id),
            PendingAction::CommitImport => self.commit_import(),
            PendingAction::Close => self.close_after_confirmation = true,
            PendingAction::CreateBuild(kind) => self.create_build_copy(kind),
            PendingAction::Route(route) => self.change_route(route),
            PendingAction::CreateCharacter => self.create_character(),
            PendingAction::OpenXp(id) => self.open_xp(id),
            PendingAction::RefreshXp => {
                if let Some(view) = &self.xp_view {
                    self.open_xp(view.record_id.clone());
                }
            }
            PendingAction::RosterXp => self.open_xp_roster(),
            PendingAction::SaveMenu(menu) => self.save_menu_preview(*menu),
            PendingAction::SelectMenu(hash) => self.select_library_menu(hash),
        }
    }

    fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Inconnu");
            ui.label(concat!("Working Build ", env!("CARGO_PKG_VERSION")));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let portuguese = self.language == Language::Portuguese;
                let pt = ui.selectable_label(portuguese, "PT");
                self.smoke_controls.portuguese = Some(pt.rect);
                if pt.clicked() {
                    self.language = Language::Portuguese;
                }
                let en = ui.selectable_label(!portuguese, "EN");
                self.smoke_controls.english = Some(en.rect);
                if en.clicked() {
                    self.language = Language::English;
                }
                ui.separator();
                if self.library.is_some()
                    && matches!(self.route, WorkArea::Player | WorkArea::Storyteller)
                    && ui
                        .button(self.t("Game information…", "Informações do jogo…"))
                        .clicked()
                {
                    self.open_chronicle();
                }
                if ui
                    .button(self.t("Open library…", "Abrir biblioteca…"))
                    .clicked()
                {
                    self.request_action(PendingAction::OpenLibrary);
                }
                if ui
                    .button(self.t("New library…", "Nova biblioteca…"))
                    .clicked()
                {
                    self.request_action(PendingAction::NewLibrary);
                }
            });
        });

        ui.horizontal(|ui| {
            if self.route == WorkArea::Storyteller
                && ui.button(self.t("Diagnostics…", "Diagnósticos…")).clicked()
            {
                self.support.open = true;
            }
            self.render_recent_libraries(ui);
            if let Some(library) = self.library.as_ref() {
                ui.strong(&library.title);
                ui.separator();
                ui.add(egui::Label::new(library.path.display().to_string()).truncate())
                    .on_hover_text(library.path.display().to_string());
                if let Some(revision) = self.revision {
                    ui.separator();
                    ui.label(format!("{} {revision}", self.t("Revision", "Revisão")));
                }
            } else {
                ui.label(self.t(
                    "No local library is open",
                    "Nenhuma biblioteca local está aberta",
                ));
            }
        });
    }

    fn render_routes(&mut self, ui: &mut egui::Ui) {
        let mut next = None;
        ui.horizontal_wrapped(|ui| {
            for (route, english, portuguese) in [
                (WorkArea::Player, "Player", "Jogador"),
                (WorkArea::Storyteller, "Storyteller", "Narrador"),
                (WorkArea::Builder, "Builder", "Criador"),
                (WorkArea::BuildTest, "Build Test", "Teste de Build"),
            ] {
                let response =
                    ui.selectable_label(self.route == route, self.t(english, portuguese));
                match route {
                    WorkArea::Player => self.smoke_controls.player = Some(response.rect),
                    WorkArea::Storyteller => self.smoke_controls.storyteller = Some(response.rect),
                    WorkArea::Builder => self.smoke_controls.builder = Some(response.rect),
                    WorkArea::BuildTest => self.smoke_controls.build_test = Some(response.rect),
                }
                if response.clicked() {
                    next = Some(route);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.route == WorkArea::Storyteller {
                    let owner = ui.label(self.t("Local library owner", "Proprietário da biblioteca local"));
                    owner.on_hover_text(self.t(
                        "Local work mode. It is not user authentication or network access control.",
                        "Modo de trabalho local. Não é autenticação de usuário nem controle de acesso à rede.",
                    ));
                } else if self.route == WorkArea::Player {
                    ui.label(self.t("Read-only view", "Visualização somente leitura"));
                }
            });
        });
        if let Some(route) = next {
            self.request_action(PendingAction::Route(route));
        }
    }

    fn render_workspace(&mut self, ui: &mut egui::Ui) {
        match self.route {
            WorkArea::Player => self.render_records(ui, false),
            WorkArea::Storyteller => self.render_records(ui, true),
            WorkArea::Builder | WorkArea::BuildTest => {
                self.render_build_tools(ui);
                self.render_saved_advancement_button(ui);
                self.render_records(ui, true);
            }
        }
    }

    fn render_build_tools(&mut self, ui: &mut egui::Ui) {
        ui.label(self.t(
            "Private build copies. Changes do not spend XP or approve a character.",
            "Cópias privadas. Alterações não gastam XP nem aprovam personagens.",
        ));
        let sources = self
            .library
            .as_ref()
            .and_then(|l| l.summaries(Role::Storyteller).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.build.is_none() && grapevine_workbench::sheet::is_character_kind(&r.kind))
            .collect::<Vec<_>>();
        if self
            .build_source_id
            .as_ref()
            .is_none_or(|id| !sources.iter().any(|r| &r.id == id))
        {
            self.build_source_id = sources.first().map(|r| r.id.clone());
        }
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("build_source")
                .selected_text(
                    sources
                        .iter()
                        .find(|r| Some(&r.id) == self.build_source_id.as_ref())
                        .map(|r| r.name.as_str())
                        .unwrap_or("—"),
                )
                .show_ui(ui, |ui| {
                    for r in &sources {
                        ui.selectable_value(&mut self.build_source_id, Some(r.id.clone()), &r.name);
                    }
                });
            let create = ui.add_enabled(
                self.build_source_id.is_some(),
                egui::Button::new(self.t("Create separate copy", "Criar cópia separada")),
            );
            self.smoke_controls.create_build = Some(create.rect);
            if create.clicked() {
                self.request_action(PendingAction::CreateBuild(
                    if self.route == WorkArea::Builder {
                        BuildKind::Draft
                    } else {
                        BuildKind::Test
                    },
                ));
            }
        });
    }
    fn render_saved_advancement_button(&mut self, ui: &mut egui::Ui) {
        if self.route == WorkArea::Builder
            && self.selected_record.as_ref().is_some_and(|r| {
                r.fields
                    .contains_key(grapevine_workbench::mixed_advancement_draft::FIELD)
            })
            && ui
                .add_enabled(
                    !self.has_unsaved_edit(),
                    egui::Button::new(self.t(
                        "Review combined spending plan…",
                        "Revisar plano combinado de gastos…",
                    )),
                )
                .clicked()
        {
            self.open_mixed_advancement();
        }
        if self.route == WorkArea::Builder
            && self.selected_record.as_ref().is_some_and(|record| {
                record
                    .fields
                    .contains_key(grapevine_workbench::advancement_draft::FIELD)
            })
            && ui
                .add_enabled(
                    !self.has_unsaved_edit(),
                    egui::Button::new(
                        self.t("Review saved advancement…", "Revisar evolução salva…"),
                    ),
                )
                .clicked()
        {
            self.open_advancement_draft();
        }
    }
    fn create_build_copy(&mut self, kind: BuildKind) {
        let (Some(library), Some(id), Some(revision)) =
            (&mut self.library, &self.build_source_id, self.revision)
        else {
            return;
        };
        match library.create_build(id, kind, revision, Role::Storyteller) {
            Ok(id) => {
                self.revision = library.revision().ok();
                self.sheet_edit = None;
                self.saved_sheet = None;
                self.edit = Edit::default();
                self.saved_edit = Edit::default();
                self.refresh_summaries(false);
                self.select_record(id);
            }
            Err(error) => self.log_error("create_build", &error),
        }
    }

    fn render_records(&mut self, ui: &mut egui::Ui, editable: bool) {
        if self.library.is_none() {
            ui.add_space(30.0);
            ui.heading(self.t(
                "Open or create a library to begin",
                "Abra ou crie uma biblioteca para começar",
            ));
            ui.label(self.t(
                "Inconnu keeps this library on your local computer. Use the controls above to open or create one.",
                "O Inconnu mantém esta biblioteca no seu computador. Use os controles acima para abrir ou criar uma.",
            ));
            return;
        }

        if editable && self.route == WorkArea::Storyteller {
            self.render_storyteller_tools(ui);
            self.render_create_character_dialog(ui.ctx());
        }
        ui.horizontal(|ui| {
            if ui.button(self.t("Refresh", "Atualizar")).clicked() {
                self.request_action(PendingAction::Refresh);
            }
        });
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label(self.t("Find", "Buscar"));
            ui.add(egui::TextEdit::singleline(&mut self.query).desired_width(220.0));
            if self.player_filter.is_some()
                && ui
                    .button(self.t("Clear player filter", "Limpar filtro de jogador"))
                    .clicked()
            {
                self.player_filter = None;
            }
        });

        let available = ui.available_size();
        let left_width = (available.x * 0.34).clamp(245.0, 340.0);
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(left_width, available.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.render_record_list(ui),
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                egui::vec2((available.x - left_width - 12.0).max(280.0), available.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.render_record_detail(ui, editable),
            );
        });
    }

    fn render_storyteller_tools(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(self.t("Items / locations / rotes…", "Itens / locais / rotinas…"))
                .clicked()
            {
                self.world.open = true;
            }
            if ui.button(self.t("Plots…", "Tramas…")).clicked() {
                if self.plot.revision_is_set() {
                    self.plot.open = true;
                } else {
                    self.load_plot(None);
                }
            }
            if ui.button(self.t("Actions…", "Ações…")).clicked() {
                self.open_story(grapevine_workbench::story_work::Kind::Action);
            }
            if ui.button(self.t("Rumors…", "Rumores…")).clicked() {
                self.open_story(grapevine_workbench::story_work::Kind::Rumor);
            }
            if ui
                .button(self.t("Rumor audience…", "Público dos rumores…"))
                .clicked()
            {
                self.open_rumor_audience();
            }
            if ui
                .button(self.t("Character associations…", "Vínculos do personagem…"))
                .clicked()
            {
                self.open_associations();
            }
            if ui
                .button(self.t("Operational reports…", "Relatórios operacionais…"))
                .clicked()
            {
                self.open_operations();
            }
            if ui
                .button(self.t("Player profiles…", "Perfis de Jogadores…"))
                .clicked()
            {
                self.open_profiles();
            }
            if ui
                .button(self.t(
                    "Saved searches / statistics…",
                    "Buscas salvas / estatísticas…",
                ))
                .clicked()
            {
                self.open_searches();
            }
            if ui
                .button(self.t("Vampire Boons…", "Favores de Vampiros…"))
                .clicked()
            {
                self.open_boons();
            }
            let points =
                ui.button(self.t("Players / Player points…", "Jogadores / pontos do Jogador…"));
            let visible = ui.is_rect_visible(points.rect).then_some(points.rect);
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::unique("points_open"), visible));
            if points.clicked() {
                self.open_points();
            }
            let response = ui.button(self.t("Game sessions…", "Sessões de jogo…"));
            let visible = ui.is_rect_visible(response.rect).then_some(response.rect);
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::unique("sessions_open"), visible));
            if response.clicked() {
                self.open_sessions();
            }
            if ui
                .button(self.t("Roster XP audit", "Auditoria de XP do elenco"))
                .clicked()
            {
                self.request_action(PendingAction::RosterXp);
            }

            if ui
                .button(self.t("New character…", "Novo personagem…"))
                .clicked()
            {
                self.create_character_open = true;
            }
            if ui
                .button(self.t(
                    "Preview game / exchange / menus…",
                    "Pré-visualizar jogo / intercâmbio / menus…",
                ))
                .clicked()
            {
                self.choose_import_file();
            }
            egui::ComboBox::from_id_salt("legacy_import_encoding")
                .selected_text(&self.import_encoding)
                .show_ui(ui, |ui| {
                    for encoding in ["windows-1252", "windows-1251", "utf-8"] {
                        ui.selectable_value(
                            &mut self.import_encoding,
                            encoding.to_owned(),
                            encoding,
                        );
                    }
                });
            if ui.button(self.t("Backup…", "Criar cópia…")).clicked() {
                self.backup_library();
            }
            if ui.button(self.t("Restore…", "Restaurar…")).clicked() {
                self.restore_library_dialog();
            }
        });
        self.render_library_menus(ui);
        self.render_import_preview(ui);
        self.render_menu_preview(ui);
    }

    fn refresh_menu_sources(&mut self) {
        self.menu_sources.clear();
        self.active_menu_hash = None;
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(library) = &self.library else { return };
        let result = (
            library.menu_catalogues(Role::Storyteller),
            library.active_menu(Role::Storyteller),
        );
        match result {
            (Ok(sources), Ok(menu)) => {
                self.menu_sources = sources;
                self.active_menu_hash = menu.as_ref().map(|m| m.sha256.clone());
                self.menu_preview = menu;
            }
            (Err(e), _) | (_, Err(e)) => self.log_error("menu.library", &e),
        }
    }
    fn finish_menu_change(&mut self, result: grapevine_workbench::model::Result<i64>) {
        match result {
            Ok(revision) => {
                self.error = None;
                self.revision = Some(revision);
                self.refresh_summaries(true);
                self.refresh_menu_sources();
                self.status = Some(
                    self.t(
                        "Library menu choice saved; no character or XP changes.",
                        "Seleção de menu salva; nenhuma alteração de personagem ou XP.",
                    )
                    .into(),
                );
            }
            Err(e) => self.log_error("menu.save", &e),
        }
    }
    fn save_menu_preview(&mut self, menu: grapevine_workbench::menus::MenuPreview) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let (Some(library), Some(revision)) = (&mut self.library, self.revision) else {
            return;
        };
        let result = library.save_select_menu(&menu, revision, Role::Storyteller);
        self.finish_menu_change(result);
    }
    fn select_library_menu(&mut self, hash: Option<String>) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let (Some(library), Some(revision)) = (&mut self.library, self.revision) else {
            return;
        };
        let result = match hash {
            Some(hash) => library.select_saved_menu(&hash, revision, Role::Storyteller),
            None => library.clear_selected_menu(revision, Role::Storyteller),
        };
        self.menu_query.clear();
        self.finish_menu_change(result);
    }
    fn render_library_menus(&mut self, ui: &mut egui::Ui) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let pt = self.language == Language::Portuguese;
        let current = self.active_menu_hash.clone();
        let mut choice = current.clone();
        let label = self
            .menu_sources
            .iter()
            .find(|m| Some(&m.sha256) == current.as_ref())
            .map(|m| m.source_name.as_str())
            .unwrap_or(if pt {
                "Nenhum menu selecionado"
            } else {
                "No menu selected"
            });
        let mut open = false;
        ui.horizontal(|ui| {
            ui.label(if pt {
                "Menu da biblioteca"
            } else {
                "Library menu"
            });
            egui::ComboBox::from_id_salt("library_menu_choice")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut choice,
                        None,
                        if pt {
                            "Nenhum menu selecionado"
                        } else {
                            "No menu selected"
                        },
                    );
                    for menu in &self.menu_sources {
                        ui.selectable_value(
                            &mut choice,
                            Some(menu.sha256.clone()),
                            format!("{} · {}", menu.source_name, menu.source_format),
                        );
                    }
                });
            if current.is_some() {
                open = ui
                    .button(if pt {
                        "Abrir menu selecionado"
                    } else {
                        "Open selected menu"
                    })
                    .clicked();
            }
        });
        if choice != current {
            self.request_action(PendingAction::SelectMenu(choice));
        } else if open {
            self.refresh_menu_sources();
        }
    }

    fn render_menu_preview(&mut self, ui: &mut egui::Ui) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(menu) = self.menu_preview.as_ref() else {
            return;
        };
        let pt = self.language == Language::Portuguese;
        let mut close = false;
        let mut save = false;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("{} · {}",menu.source_format,menu.source_name));
                let button=ui.small_button("×");
                self.smoke_controls.menu_close=Some(button.rect);
                close=button.clicked();
            });
            ui.label(format!("{}: {}",if pt {"Entradas"} else {"Entries"},menu.rows.len()));
            egui::CollapsingHeader::new(if pt {"Identificação da origem"} else {"Source identification"}).show(ui,|ui| {ui.small(format!("SHA256 {}",menu.sha256));});
            ui.weak(if pt {"Definições legadas somente para leitura. Custos e notas não verificados; nenhum dado de personagem ou XP é importado."} else {"Read-only legacy definitions. Costs and notes are unverified; no character data or XP is imported."});
            let button=ui.button(if pt {"Salvar e usar este menu"} else {"Save and use this menu"});
            self.smoke_controls.menu_save=Some(button.rect);save=button.clicked();
            ui.small(if pt {"A seleção salva apenas a origem do menu. Custos continuam não verificados."} else {"Saving selects catalogue data only; costs remain unverified."});
            let input=ui.add(egui::TextEdit::singleline(&mut self.menu_query).hint_text(if pt {"Buscar nomes de menu"} else {"Search menu names"}));
            self.smoke_controls.menu_search=Some(input.rect);
            let query=self.menu_query.to_lowercase();
            egui::ScrollArea::vertical().id_salt("menu_source_rows").max_height(240.0).show(ui,|ui| {
                for row in menu.rows.iter().filter(|r|r.path.join(" / ").to_lowercase().contains(&query)).take(200) {
                    ui.label(format!("{} · {}",row.kind,row.path.join(" / ")));
                    if let Some(cost)=row.attributes.get("cost") {ui.small(format!("{}: {cost}",if pt {"Custo original"} else {"Raw cost"}));}
                    if let Some(note)=row.attributes.get("note") {ui.small(format!("{}: {note}",if pt {"Nota original"} else {"Raw note"}));}
                }
            });
            ui.small(if pt {"Primeiras200 correspondências; busque para filtrar. Arquivos originais preservados."} else {"First200 matches; search narrows the list. Original files remain untouched."});
        });
        if close {
            self.menu_preview = None;
            self.menu_query.clear();
        } else if save && let Some(menu) = self.menu_preview.clone() {
            self.request_action(PendingAction::SaveMenu(Box::new(menu)));
        }
    }

    fn render_import_preview(&mut self, ui: &mut egui::Ui) {
        let Some(preview) = self.import_preview.as_ref() else {
            return;
        };
        let source_name = preview.source_name.clone();
        let source_format = preview.source_format.clone();
        let menu_reference = preview.menu_reference.clone();
        let menu_candidate = preview.menu_candidate.clone();
        let mut open_menu = false;
        let counts = preview.counts.clone();
        let warnings = preview.warnings.clone();
        let conflicts = preview.conflicts.clone();
        let records_count = preview.records.len();
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(self.t("Import preview", "Pré-visualização da importação"));
                ui.label(source_name);
                if ui.small_button(self.t("Dismiss", "Fechar")).clicked() {
                    self.import_preview = None;
                    self.status = None;
                }
            });
            ui.label(format!("{}: {source_format}",self.t("Detected content","Conteúdo detectado")));
            if let Some(reference)=&menu_reference {
                ui.label(format!("{}: {reference}",self.t("External menu","Menu externo")));
                if menu_candidate.is_some() {
                    if ui.button(self.t("Preview referenced menu","Pré-visualizar menu referido")).clicked(){open_menu=true;}
                } else {ui.weak(self.t("Menu file not found beside this source. Its names are not embedded menu definitions.","Menu não encontrado junto à origem. Nomes da ficha não são definições de menu."));}
            }
            ui.label(format!(
                "{}: {records_count}",
                self.t("Records", "Registros")
            ));
            if !counts.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for (kind, count) in &counts {
                        ui.label(format!("{kind}: {count}"));
                    }
                });
            }
            if !warnings.is_empty() {
                ui.label(format!(
                    "{}: {}",
                    self.t("Warnings", "Avisos"),
                    warnings.len()
                ));
                egui::ScrollArea::vertical()
                    .max_height(60.0)
                    .show(ui, |ui| {
                        for warning in &warnings {
                            ui.label(format!("• {warning}"));
                        }
                    });
            }
            if !conflicts.is_empty() {
                ui.label(format!(
                    "{}: {}",
                    self.t("Conflicts", "Conflitos"),
                    conflicts.len()
                ));
                let skip_label = self.t(
                    "Skip conflicting records when committing",
                    "Ignorar registros conflitantes ao confirmar",
                );
                ui.checkbox(&mut self.skip_import_conflicts, skip_label);
                egui::CollapsingHeader::new(self.t(
                    "Review conflicting record names",
                    "Revisar nomes de registros conflitantes",
                ))
                .show(ui, |ui| {
                    for conflict in &conflicts {
                        ui.label(conflict);
                    }
                });
            }
            ui.horizontal(|ui| {
                let can_commit = conflicts.is_empty() || self.skip_import_conflicts;
                let commit = ui.add_enabled(
                    can_commit,
                    egui::Button::new(self.t("Commit import…", "Confirmar importação…")),
                );
                self.smoke_controls.commit_import = Some(commit.rect);
                if commit.clicked() {
                    self.request_action(PendingAction::CommitImport);
                }
                if !conflicts.is_empty() && !self.skip_import_conflicts {
                    ui.label(self.t(
                        "Choose to skip conflicts before committing.",
                        "Escolha ignorar os conflitos antes de confirmar.",
                    ));
                }
            });
        });
        if open_menu && let Some(path) = menu_candidate {
            match grapevine_workbench::menus::read_with_encoding(&path, &self.import_encoding) {
                Ok(Some(menu)) => {
                    self.menu_preview = Some(menu);
                    self.menu_query.clear();
                }
                Ok(None) => {
                    self.error=Some("Referenced file is not a supported XML menu; game import remains separate.".into());
                }
                Err(error) => self.log_error("menu.reference", &error),
            }
        }
    }

    fn render_create_character_dialog(&mut self, ctx: &egui::Context) {
        if !self.create_character_open {
            return;
        }
        let mut close = false;
        egui::Window::new(self.t("Create character", "Criar personagem"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(self.t("Character type", "Tipo de personagem"));
                egui::ComboBox::from_id_salt("new_character_kind")
                    .selected_text(&self.new_character_kind)
                    .show_ui(ui, |ui| {
                        for kind in [
                            "Vampire",
                            "Werewolf",
                            "Mortal",
                            "Changeling",
                            "Wraith",
                            "Mage",
                            "Fera",
                            "Various",
                            "Mummy",
                            "KueiJin",
                            "Hunter",
                            "Demon",
                            "Risen",
                        ] {
                            ui.selectable_value(
                                &mut self.new_character_kind,
                                kind.to_owned(),
                                kind,
                            );
                        }
                    });
                ui.label(self.t("Name", "Nome"));
                ui.text_edit_singleline(&mut self.new_character_name);
                ui.horizontal(|ui| {
                    if ui.button(self.t("Create", "Criar")).clicked() {
                        self.request_action(PendingAction::CreateCharacter);
                        close = true;
                    }
                    if ui.button(self.t("Cancel", "Cancelar")).clicked() {
                        close = true;
                    }
                });
            });
        if close {
            self.create_character_open = false;
        }
    }

    fn filtered_rows(&self) -> Vec<Summary> {
        let query = self.query.trim().to_lowercase();
        self.summaries
            .iter()
            .filter(|summary| {
                let player_matches = self
                    .player_filter
                    .as_ref()
                    .is_none_or(|player| summary.player_name.eq_ignore_ascii_case(player));
                let query_matches = query.is_empty()
                    || summary.name.to_lowercase().contains(&query)
                    || summary.kind.to_lowercase().contains(&query)
                    || summary.player_name.to_lowercase().contains(&query);
                player_matches && query_matches
            })
            .cloned()
            .collect()
    }

    fn render_record_list(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("Characters", "Personagens"));
        let rows = self.filtered_rows();
        egui::ScrollArea::vertical()
            .id_salt("record_list")
            .show(ui, |ui| {
                for summary in rows {
                    let selected = self.selected_id.as_deref() == Some(summary.id.as_str());
                    let label = format!(
                        "{}\n{} · {}",
                        summary.name, summary.kind, summary.player_name
                    );
                    if ui.selectable_label(selected, label).clicked() && !selected {
                        self.request_action(PendingAction::SelectRecord(summary.id));
                    }
                }
                if self.filtered_rows().is_empty() {
                    ui.label(self.t("No matching records", "Nenhum registro correspondente"));
                }
            });

        ui.separator();
        ui.heading(self.t("Players", "Jogadores"));
        let mut players: Vec<(String, usize)> = Vec::new();
        for summary in &self.summaries {
            if summary.player_name.trim().is_empty() {
                continue;
            }
            if let Some((_, count)) = players
                .iter_mut()
                .find(|(name, _)| name.eq_ignore_ascii_case(&summary.player_name))
            {
                *count += 1;
            } else {
                players.push((summary.player_name.clone(), 1));
            }
        }
        players.sort_by_key(|a| a.0.to_lowercase());
        egui::ScrollArea::vertical()
            .id_salt("player_list")
            .max_height(150.0)
            .show(ui, |ui| {
                if ui
                    .selectable_label(
                        self.player_filter.is_none(),
                        self.t("All players", "Todos os jogadores"),
                    )
                    .clicked()
                {
                    self.player_filter = None;
                }
                for (name, count) in players {
                    let selected = self
                        .player_filter
                        .as_deref()
                        .is_some_and(|filter| filter.eq_ignore_ascii_case(&name));
                    if ui
                        .selectable_label(selected, format!("{name} · {count}"))
                        .clicked()
                    {
                        self.player_filter = Some(name);
                    }
                }
            });
    }

    fn render_record_detail(&mut self, ui: &mut egui::Ui, editable: bool) {
        let Some(record) = self.selected_record.as_ref() else {
            ui.heading(self.t("No character selected", "Nenhum personagem selecionado"));
            return;
        };
        let record = record.clone();
        ui.heading(&record.name);
        ui.label(format!("{} · {}", record.kind, record.player_name));
        ui.horizontal_wrapped(|ui| {
            if grapevine_workbench::sheet::is_character_kind(&record.kind) {
                let button = ui.button(self.t(
                    "Print / export character sheet",
                    "Imprimir / exportar ficha",
                ));
                self.smoke_controls.print = Some(button.rect);
                if button.clicked() {
                    self.open_print();
                }
            }
            if editable && grapevine_workbench::sheet::is_character_kind(&record.kind) {
                if grapevine_workbench::sheet::is_character_kind(&record.kind) {
                    let b = ui
                        .button(self.t("V20 / DA20 source references…", "Referências V20 / DA20…"));
                    let rect = ui.is_rect_visible(b.rect).then_some(b.rect);
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(egui::Id::unique("published_open"), rect)
                    });
                    if b.clicked() {
                        self.open_published_sources();
                    }
                }
                let xp = ui.button(self.t("XP ledger / audit", "XP / auditoria"));
                self.smoke_controls.xp = Some(xp.rect);
                if xp.clicked() {
                    self.request_open_xp(record.id.clone());
                }
                let button = ui.button(self.t("What-if XP spending", "Simular gastos de XP"));
                self.smoke_controls.scenario = Some(button.rect);
                if button.clicked() {
                    self.open_scenarios();
                }
                if self.route == WorkArea::Storyteller && record.build.is_none() {
                    let button = ui.button(self.t(
                        "XP versus playtime / transfer review",
                        "XP por tempo de jogo / revisão de transferência",
                    ));
                    self.smoke_controls.playtime = Some(button.rect);
                    if button.clicked() {
                        self.open_playtime();
                    }
                }
                if self.route == WorkArea::Storyteller
                    && record.build.is_none()
                    && grapevine_workbench::sheet::is_character_kind(&record.kind)
                {
                    let b = ui.button(self.t("Purchase / receipts", "Compra / recibos"));
                    self.smoke_controls.purchase = Some(b.rect);
                    if b.clicked() {
                        self.open_purchases();
                    }
                    let b =
                        ui.button(self.t("Advance ordinary traits…", "Avançar características…"));
                    let rect = ui.is_rect_visible(b.rect).then_some(b.rect);
                    ui.ctx()
                        .data_mut(|data| data.insert_temp(egui::Id::unique("ordinary_open"), rect));
                    if b.clicked() {
                        self.open_trait_advancement();
                    }
                    if ui
                        .button(self.t("MET reminder mapping…", "Vínculo MET para lembrete…"))
                        .clicked()
                    {
                        self.open_met_reference();
                    }

                    if ui
                        .button(self.t(
                            "Entering play / retirement…",
                            "Entrada em jogo / aposentadoria…",
                        ))
                        .clicked()
                    {
                        self.open_character_lifecycle();
                    }
                    if ui
                        .button(self.t("Combined spending plan…", "Plano combinado de gastos…"))
                        .clicked()
                    {
                        self.open_mixed_advancement();
                    }
                    if ui
                        .button(self.t("Initial Vampire creation…", "Criação inicial de Vampiro…"))
                        .clicked()
                    {
                        self.open_vampire_creation();
                    }
                }
            }
        });

        let mut request_save = false;
        let mut request_discard = false;
        if editable {
            ui.horizontal(|ui| {
                let dirty = self.edit != self.saved_edit || self.sheet_edit != self.saved_sheet;
                let save_response = ui.add_enabled(
                    dirty,
                    egui::Button::new(self.t("Save changes", "Salvar alterações")),
                );
                self.smoke_controls.save = ui
                    .is_rect_visible(save_response.rect)
                    .then_some(save_response.rect);
                if save_response.clicked() {
                    request_save = true;
                }
                if dirty
                    && ui
                        .button(self.t("Discard draft", "Descartar rascunho"))
                        .clicked()
                {
                    request_discard = true;
                }
                if dirty {
                    ui.label(self.t("Unsaved changes", "Alterações não salvas"));
                }
            });
        }
        if self.smoke.is_some() {
            let viewport = ui.available_rect_before_wrap().intersect(ui.clip_rect());
            ui.ctx().data_mut(|data| {
                data.insert_temp(egui::Id::unique("record_detail_scroll"), Some(viewport));
            });
        }
        egui::ScrollArea::vertical()
            .id_salt("record_detail")
            .show(ui, |ui| {
                if editable {
                    ui.separator();
                    ui.heading(self.t("Storyteller edit", "Edição do Narrador"));
                    ui.label(self.t("Name", "Nome"));
                    let name_response = ui.add(
                        egui::TextEdit::singleline(&mut self.edit.name)
                            .desired_width(f32::INFINITY),
                    );
                    let visible_name=name_response.rect.intersect(ui.clip_rect());
                    self.smoke_controls.name=(visible_name.height()+0.5>=name_response.rect.height() && visible_name.width()>=24.0).then_some(visible_name);
                    ui.ctx().data_mut(|d|d.insert_temp(egui::Id::unique("sheet_name_bounds"),Some(name_response.rect)));
                    if let Some(smoke) = self.smoke.as_mut()
                        && smoke.phase == SmokePhase::TypeName
                    {
                        smoke.checks.insert(
                            "name_field_focused".into(),
                            serde_json::json!(name_response.has_focus()),
                        );
                    }
                    ui.label(self.t("Status", "Estado"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.edit.status)
                            .desired_width(f32::INFINITY),
                    );
                    ui.label(self.t(
                        "Private Storyteller notes",
                        "Anotações privadas do Narrador",
                    ));
                    ui.add(
                        egui::TextEdit::multiline(&mut self.edit.notes)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                } else {
                    ui.separator();
                    ui.heading(self.t("Status", "Estado"));
                    ui.label(if record.status.trim().is_empty() {
                        "—"
                    } else {
                        &record.status
                    });
                }

                if editable {
                    let players = self
                        .library
                        .as_ref()
                        .and_then(|l| l.summaries(Role::Storyteller).ok())
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|r| r.kind == "Player")
                        .collect::<Vec<_>>();
                    if let Some(sheet) = self.sheet_edit.as_mut() {
                        crate::sheet_ui::render(ui, sheet, self.language, &players);
                    }
                }
                if editable && !record.notes.trim().is_empty() {
                    ui.separator();
                    ui.heading(self.t("Saved Storyteller notes", "Anotações salvas do Narrador"));
                    ui.label(&record.notes);
                }
                ui.separator();
                ui.heading(self.t("Sheet fields and retained legacy evidence", "Campos da ficha e evidências legadas"));
                ui.weak(self.t("Imported Experience fields retain the old history; use XP ledger / audit for current local balances.","Campos de Experience preservam o histórico antigo; consulte XP / auditoria para saldos locais atuais."));
                render_fields(ui, &record.fields, self.language);
            });
        if request_discard {
            self.edit = self.saved_edit.clone();
            self.sheet_edit = self.saved_sheet.clone();
        } else if request_save {
            self.save_edit();
        }
    }

    fn request_open_xp(&mut self, id: String) {
        if self.route.role() != Some(Role::Storyteller) {
            return;
        }
        if self.xp_view.as_ref().is_some_and(|v| v.record_id == id) {
            self.xp_window_open = true;
            if self.xp_form == self.xp_saved_form
                && let Some(library) = &self.library
            {
                self.xp_view = library.xp_view(&id, Role::Storyteller).ok();
                self.xp_revision = library.revision().ok();
            }
        } else {
            self.request_action(PendingAction::OpenXp(id));
        }
    }
    fn open_xp(&mut self, id: String) {
        if self.route.role() != Some(Role::Storyteller) {
            return;
        }
        let Some(library) = &self.library else {
            return;
        };
        match library.xp_view(&id, Role::Storyteller) {
            Ok(view) => {
                self.xp_view = Some(view);
                self.xp_revision = library.revision().ok();
                self.xp_form = Default::default();
                if self.smoke.is_some() {
                    self.xp_form.opening_earned = "10".into();
                    self.xp_form.opening_unspent = "10".into();
                    self.xp_form.day = "2026-01-01".into();
                    self.xp_form.actor = "Synthetic ST".into();
                    self.xp_form.reason = "Synthetic opening".into();
                    self.xp_form.evidence = "SMOKE_OPENING".into();
                }
                self.xp_saved_form = self.xp_form.clone();
                self.xp_message = None;
                self.xp_window_open = true;
            }
            Err(error) => self.log_error("xp_open", &error),
        }
    }
    fn open_xp_roster(&mut self) {
        if self.route.role() != Some(Role::Storyteller) {
            return;
        }
        let Some(library) = &self.library else {
            return;
        };
        match library.xp_roster_audit(Role::Storyteller) {
            Ok(views) => self.xp_roster = Some(views),
            Err(error) => self.log_error("xp_roster", &error),
        }
    }
    fn xp_action(&mut self, action: crate::xp_ui::XpAction) {
        if self.route.role() != Some(Role::Storyteller) {
            return;
        }
        if action == crate::xp_ui::XpAction::ExportAudit {
            let id = self.xp_view.as_ref().map(|v| v.record_id.clone());
            self.export_xp_audit(id.as_deref());
            return;
        }
        let (Some(library), Some(id), Some(expected)) = (
            &mut self.library,
            self.xp_view.as_ref().map(|v| v.record_id.clone()),
            self.xp_revision,
        ) else {
            return;
        };
        let result = match action {
            crate::xp_ui::XpAction::Initialize(b) => {
                library.initialize_xp(&id, &b, expected, Role::Storyteller)
            }
            crate::xp_ui::XpAction::Post(c) => {
                library.post_xp(&id, &c, expected, Role::Storyteller)
            }
            _ => return,
        };
        match result {
            Ok(revision) => {
                self.revision = Some(revision);
                self.xp_revision = Some(revision);
                self.xp_view = library.xp_view(&id, Role::Storyteller).ok();
                let actor = self.xp_form.actor.clone();
                let day = self.xp_form.day.clone();
                self.xp_form = Default::default();
                self.xp_form.actor = actor;
                self.xp_form.day = day;
                self.xp_saved_form = self.xp_form.clone();
                self.error = None;
                self.xp_message = Some(
                    self.t(
                        "Entry saved. Audit evidence retained.",
                        "Registro salvo. Evidências preservadas.",
                    )
                    .into(),
                );
            }
            Err(error) => {
                self.log_error("xp_post", &error);
                self.xp_message = self.error.clone();
            }
        }
    }
    fn open_print(&mut self) {
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        if self.print_target.as_ref() != Some(&id) {
            self.print_form = Default::default();
        }
        self.print_target = Some(id);
        self.print_open = true;
    }
    fn render_print(&mut self, ctx: &egui::Context) {
        if !self.print_open {
            return;
        }
        let (Some(lib), Some(id), Some(role)) =
            (&self.library, &self.print_target, self.active_role())
        else {
            return;
        };
        if self.selected_id.as_ref() != Some(id) {
            self.print_open = false;
            return;
        }
        let revision = lib.revision().unwrap_or(-1);
        let mut open = true;
        let mut action = None;
        egui::Window::new(self.t("Printable character sheet", "Ficha para impressão"))
            .id(egui::Id::unique("print_window"))
            .default_size([780.0, 650.0])
            .open(&mut open)
            .show(ctx, |ui| {
                action = crate::print_ui::render(
                    ui,
                    &mut self.print_form,
                    revision,
                    self.language,
                    role == Role::Storyteller,
                );
            });
        self.print_open = open;
        match action {
            Some(crate::print_ui::Action::Preview) => {
                match lib.print_document(id, &self.print_form.options, revision, role) {
                    Ok(doc) => {
                        self.print_form.document = Some(doc);
                        self.print_form.revision = Some(revision);
                        self.print_form.message = None;
                    }
                    Err(e) => {
                        self.print_form.document = None;
                        self.print_form.message = Some(e.message);
                    }
                }
            }
            Some(crate::print_ui::Action::Export) => {
                if self.print_form.revision != Some(revision) {
                    return;
                }
                let Some(doc) = &self.print_form.document else {
                    return;
                };
                if doc.options != self.print_form.options {
                    return;
                }
                if let Some(path) = FileDialog::new()
                    .set_file_name("Character sheet.html")
                    .add_filter("Printable HTML", &["html"])
                    .save_file()
                {
                    if lib.revision().ok() != Some(revision) {
                        self.print_form.message =
                            Some("Library changed; preview again before exporting.".into());
                        return;
                    }
                    let result = grapevine_workbench::printing::html(doc).and_then(|html| {
                        grapevine_workbench::safe_files::write_new(&path, html.as_bytes())
                    });
                    self.print_form.message=Some(match result {Ok(())=>self.t("Printable HTML saved; open it in your browser to print or save as PDF.","HTML salvo; abra no navegador para imprimir ou salvar em PDF.").into(),Err(e)=>e.message});
                }
            }
            Some(crate::print_ui::Action::ExportMobile) => {
                use grapevine_workbench::mobile_bundle::EXTENSION;
                if let Some(mut path) = FileDialog::new()
                    .set_file_name(format!("Character{EXTENSION}"))
                    .add_filter("JSON", &["json"])
                    .save_file()
                {
                    if let Some(name) = path.file_name().filter(|name| {
                        !name
                            .to_string_lossy()
                            .to_ascii_lowercase()
                            .ends_with(EXTENSION)
                    }) {
                        let stem = name.to_string_lossy();
                        let stem = if stem.to_ascii_lowercase().ends_with(".json") {
                            &stem[..stem.len() - 5]
                        } else {
                            &stem
                        };
                        path.set_file_name(format!("{stem}{EXTENSION}"));
                    }
                    let result = lib.write_mobile_sheet(
                        id,
                        self.print_form.options.language,
                        revision,
                        &path,
                    );
                    self.print_form.message = Some(match result {
                        Ok(()) => self
                            .t(
                                "Player-safe mobile sheet saved.",
                                "Ficha móvel do jogador salva.",
                            )
                            .into(),
                        Err(error) => error.message,
                    });
                }
            }
            Some(crate::print_ui::Action::PreviewGex) => {
                self.print_form.gex_preview = None;
                match lib.gex_preview(
                    self.print_form.gex_target,
                    Some(id),
                    &self.print_form.gex_options,
                    revision,
                    role,
                ) {
                    Ok(preview) => {
                        self.print_form.gex_preview = Some(preview);
                        self.print_form.message = None;
                    }
                    Err(e) => self.print_form.message = Some(e.message),
                }
            }
            Some(crate::print_ui::Action::ExportGex) => {
                let Some(preview) = &self.print_form.gex_preview else {
                    return;
                };
                if preview.options != self.print_form.gex_options
                    || preview.target != self.print_form.gex_target
                    || preview.revision != revision
                {
                    return;
                }
                if let Some(path) = FileDialog::new()
                    .set_file_name("Inconnu transfer.gex")
                    .add_filter("Grapevine XML", &["gex"])
                    .save_file()
                {
                    let result = lib.write_gex(preview, &path, role);
                    self.print_form.message = Some(match result {
                        Ok(()) => self
                            .t(
                                "Legacy XML saved. Review the export warnings before transferring.",
                                "XML legado salvo. Revise os avisos antes de transferir.",
                            )
                            .into(),
                        Err(e) => e.message,
                    });
                }
            }
            None => {}
        }
    }
    fn open_playtime(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        if self.playtime_target.as_ref() != Some(&id) {
            self.playtime_form = Default::default();
            if let Some(lib) = &self.library
                && let Ok(view) = lib.xp_view(&id, Role::Storyteller)
                && let Some(ledger) = view.ledger
            {
                self.playtime_form.input.start_month =
                    ledger.baseline.day.get(..7).unwrap_or("").into();
                self.playtime_form.input.end_month = self.playtime_form.input.start_month.clone();
            }
        }
        self.playtime_target = Some(id.clone());
        if let Some(lib) = &self.library
            && let Ok(revision) = lib.revision()
        {
            match lib.playtime_awards(
                &id,
                &self.playtime_form.input.start_month,
                &self.playtime_form.input.end_month,
                revision,
                Role::Storyteller,
            ) {
                Ok(rows) => self.playtime_form.load_awards(rows, revision),
                Err(e) => {
                    self.playtime_form.award_revision = None;
                    self.playtime_form.invalidate();
                    self.playtime_form.message = Some(e.message);
                }
            }
        }
        if let Some(lib) = &self.library {
            match lib.playtime_award_reviews(&id, Role::Storyteller) {
                Ok(reviews) => {
                    self.playtime_form.saved_reviews = reviews;
                    if self
                        .playtime_form
                        .selected_review
                        .as_ref()
                        .is_none_or(|id| {
                            !self.playtime_form.saved_reviews.iter().any(|r| &r.id == id)
                        })
                    {
                        self.playtime_form.selected_review = self
                            .playtime_form
                            .saved_reviews
                            .iter()
                            .max_by_key(|r| r.saved_revision)
                            .map(|r| r.id.clone());
                    }
                }
                Err(e) => self.playtime_form.message = Some(e.message),
            }
        }
        self.playtime_open = true;
    }
    fn render_playtime(&mut self, ctx: &egui::Context) {
        if !self.playtime_open || self.route != WorkArea::Storyteller {
            return;
        }
        let (Some(lib), Some(id)) = (&self.library, &self.playtime_target) else {
            return;
        };
        if self.selected_id.as_ref() != Some(id) {
            self.playtime_open = false;
            return;
        }
        let revision = lib.revision().unwrap_or(-1);
        let mut open = true;
        let mut action = None;
        egui::Window::new(self.t("XP versus playtime", "XP por tempo de jogo"))
            .id(egui::Id::unique("playtime_window"))
            .default_size([790.0, 640.0])
            .open(&mut open)
            .show(ctx, |ui| {
                action = crate::playtime_ui::render(
                    ui,
                    &mut self.playtime_form,
                    revision,
                    self.language,
                );
            });
        self.playtime_open = open;
        match action {
            Some(crate::playtime_ui::Action::SavePrivate) => {
                let review_id = self
                    .playtime_form
                    .save_review_id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let result = self.library.as_mut().unwrap().save_playtime_award_review(
                    id,
                    &review_id,
                    &self.playtime_form.input,
                    revision,
                    Role::Storyteller,
                );
                match result {
                    Ok(saved) => {
                        let lib = self.library.as_ref().unwrap();
                        let next = lib.revision().unwrap_or(revision);
                        self.revision = Some(next);
                        match lib.playtime_award_reviews(id, Role::Storyteller) {
                            Ok(reviews) => self.playtime_form.saved_reviews = reviews,
                            Err(e) => {
                                self.playtime_form.message = Some(e.message);
                                return;
                            }
                        }
                        self.playtime_form.selected_review = Some(saved.id.clone());
                        self.playtime_form.save_review_id = Some(saved.id);
                        self.playtime_form.award_revision = Some(next);
                        self.playtime_form.invalidate();
                        self.playtime_form.message = Some(
                            self.t(
                                "Private choices saved with the library. XP is unchanged.",
                                "Escolhas privadas salvas com a biblioteca. O XP não mudou.",
                            )
                            .into(),
                        );
                    }
                    Err(e) => self.playtime_form.message = Some(e.message),
                }
            }
            Some(crate::playtime_ui::Action::LoadPrivate(review_id)) => {
                match lib.load_playtime_award_review(id, &review_id, revision, Role::Storyteller) {
                    Ok(loaded) => {
                        match lib.playtime_awards(
                            id,
                            &loaded.input.start_month,
                            &loaded.input.end_month,
                            revision,
                            Role::Storyteller,
                        ) {
                            Ok(rows) => {
                                let same = loaded.input.award_reviews == loaded.saved.award_reviews;
                                self.playtime_form.input = loaded.input;
                                self.playtime_form.load_awards(rows, revision);
                                self.playtime_form.save_review_id =
                                    same.then_some(loaded.saved.id.clone());
                                self.playtime_form.selected_review = Some(loaded.saved.id);
                                self.playtime_form.classification_reason.clear();
                                self.playtime_form.classification_reference.clear();
                                self.playtime_form.message=Some(format!("{}\n{}: {}\n{}: {}",self.t("Choices loaded. Confirm policy and eligibility again.","Escolhas carregadas. Confirme novamente a regra e a elegibilidade."),self.t("Saved choices that no longer match","Escolhas salvas que não correspondem mais"),loaded.stale_reviews,self.t("Unclassified current awards","Concessões atuais não classificadas"),loaded.unclassified_awards));
                            }
                            Err(e) => self.playtime_form.message = Some(e.message),
                        }
                    }
                    Err(e) => self.playtime_form.message = Some(e.message),
                }
            }
            Some(crate::playtime_ui::Action::LoadAwards) => {
                match lib.playtime_awards(
                    id,
                    &self.playtime_form.input.start_month,
                    &self.playtime_form.input.end_month,
                    revision,
                    Role::Storyteller,
                ) {
                    Ok(rows) => self.playtime_form.load_awards(rows, revision),
                    Err(e) => {
                        self.playtime_form.award_revision = None;
                        self.playtime_form.invalidate();
                        self.playtime_form.message = Some(e.message);
                    }
                }
            }
            Some(crate::playtime_ui::Action::Calculate) => {
                match lib.playtime_report(
                    id,
                    &self.playtime_form.input,
                    revision,
                    Role::Storyteller,
                ) {
                    Ok(report) => {
                        self.playtime_form.report = Some(report);
                        self.playtime_form.report_input = Some(self.playtime_form.input.clone());
                        self.playtime_form.report_revision = Some(revision);
                        self.playtime_form.message = None;
                    }
                    Err(e) => {
                        self.playtime_form.report = None;
                        self.playtime_form.message = Some(e.message);
                    }
                }
            }
            Some(crate::playtime_ui::Action::Export) => {
                if self.playtime_form.report_revision != Some(revision)
                    || self.playtime_form.report_input.as_ref() != Some(&self.playtime_form.input)
                {
                    return;
                }
                if let Some(value) = &self.playtime_form.report
                    && let Some(path) = FileDialog::new()
                        .set_file_name("Inconnu-Transfer-Review.json")
                        .add_filter("Review summary", &["json"])
                        .save_file()
                {
                    if lib.revision().ok() != Some(revision) {
                        self.playtime_form.message =
                            Some("Library changed; recalculate before exporting.".into());
                        return;
                    }
                    let result = serde_json::to_vec_pretty(value)
                        .map_err(Into::into)
                        .and_then(|bytes| {
                            grapevine_workbench::safe_files::write_new(&path, &bytes)
                        });
                    self.playtime_form.message = Some(match result {
                        Ok(()) => self
                            .t("Review summary saved", "Resumo de revisão salvo")
                            .into(),
                        Err(e) => e.message,
                    });
                }
            }
            None => {}
        }
    }
    fn clear_purchases(&mut self) {
        self.met_reference = Default::default();
        self.lifecycle = Default::default();
        self.mixed = Default::default();
        self.legacy_profile = Default::default();
        self.legacy_calendar = Default::default();
        self.creation = Default::default();
        self.ordinary = Default::default();
        self.published = Default::default();
        self.advancement = Default::default();
        self.support = Default::default();
        self.rumor_audience = Default::default();
        self.world = Default::default();
        self.plot = Default::default();
        self.story = Default::default();
        self.associations = Default::default();
        self.operations = Default::default();
        self.profiles = Default::default();
        self.chronicle = Default::default();
        self.boons = Default::default();
        self.searches = Default::default();
        self.points_open = false;
        self.points_target = None;
        self.points_form = Default::default();
        self.points_ledger = None;
        self.points_revision = None;
        self.points_saved_entry = serde_json::Value::Null;
        self.session_open = false;
        self.session_form = Default::default();
        self.print_open = false;
        self.print_target = None;
        self.print_form = Default::default();
        self.playtime_open = false;
        self.playtime_target = None;
        self.playtime_form = Default::default();
        self.purchase_open = false;
        self.purchase_target = None;
        self.purchase_form = Default::default();
    }
    fn open_purchases(&mut self) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        let (Some(lib), Some(id)) = (&self.library, &self.selected_id) else {
            return;
        };
        match lib.purchase_receipts(id, Role::Storyteller) {
            Ok(receipts) => {
                if self.purchase_target.as_ref() != Some(id) {
                    self.purchase_form = Default::default();
                }
                self.purchase_form.kind = self
                    .selected_record
                    .as_ref()
                    .map(|r| r.kind.clone())
                    .unwrap_or_else(|| "Vampire".into());
                self.purchase_target = Some(id.clone());
                self.purchase_form.receipts = receipts;
                match lib.purchase_reviews(id, Role::Storyteller) {
                    Ok(reviews) => self.purchase_form.reviews = reviews,
                    Err(e) => {
                        self.log_error("purchase.reviews", &e);
                        return;
                    }
                }
                self.purchase_open = true;
            }
            Err(e) => self.log_error("purchase.open", &e),
        }
    }
    fn load_policy_path(
        &mut self,
        path: &std::path::Path,
    ) -> grapevine_workbench::model::Result<()> {
        use std::io::Read;
        if self.route != WorkArea::Storyteller || self.purchase_form.cancel_purchase.is_some() {
            return Err(grapevine_workbench::model::error(
                "ACCESS_DENIED",
                "Open a local Storyteller purchase form first.",
            ));
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(131073)
            .read_to_end(&mut bytes)?;
        let template = grapevine_workbench::policy_templates::PolicyTemplate::parse(&bytes)?;
        template.apply_for_kind(&mut self.purchase_form.input, &self.purchase_form.kind)?;
        self.purchase_form.policy_name = template.name;
        self.purchase_form.quote = None;
        self.purchase_form.dirty = true;
        self.purchase_form.message = Some(self.t("Policy loaded. Review fresh approval, eligibility and teacher evidence before quoting.", "Regra carregada. Revise nova aprovação, elegibilidade e evidência do professor antes de cotar.").into());
        Ok(())
    }
    fn save_policy_path(&self, path: &std::path::Path) -> grapevine_workbench::model::Result<()> {
        if self.route != WorkArea::Storyteller || self.purchase_form.cancel_purchase.is_some() {
            return Err(grapevine_workbench::model::error(
                "ACCESS_DENIED",
                "Open a local Storyteller purchase form first.",
            ));
        }
        let rules = grapevine_workbench::creature_profiles::profile(&self.purchase_form.kind)
            .ok_or_else(|| {
                grapevine_workbench::model::error(
                    "POLICY_TEMPLATE_SCOPE",
                    "Select a supported character.",
                )
            })?;
        let template =
            grapevine_workbench::policy_templates::PolicyTemplate::from_request_for_kind(
                &self.purchase_form.policy_name,
                &self.purchase_form.kind,
                &self.purchase_form.input,
                rules,
            )?;
        let bytes = template.to_bytes()?;
        grapevine_workbench::safe_files::write_new(path, &bytes)
    }
    fn purchase_action(&mut self, action: crate::purchase_ui::Action) {
        if self.route != WorkArea::Storyteller {
            return;
        }
        if matches!(
            action,
            crate::purchase_ui::Action::SavePolicy | crate::purchase_ui::Action::LoadPolicy
        ) {
            if self.purchase_form.cancel_purchase.is_some() {
                return;
            }
            let save = matches!(action, crate::purchase_ui::Action::SavePolicy);
            let dialog = FileDialog::new().add_filter("Inconnu local policy", &["gvpolicy"]);
            let path = if save {
                dialog.set_file_name("Local policy.gvpolicy").save_file()
            } else {
                dialog.pick_file()
            };
            if let Some(path) = path {
                let result = if save {
                    self.save_policy_path(&path)
                } else {
                    self.load_policy_path(&path)
                };
                match result {
                    Ok(()) if save => self.purchase_form.message = Some(self.t("Policy template saved. Choose a new filename to save another version.", "Modelo de regra salvo. Escolha outro nome de arquivo para salvar outra versão.").into()),
                    Ok(()) => {},
                    Err(e) => self.purchase_form.message = Some(e.message),
                }
            }
            return;
        }
        if matches!(action, crate::purchase_ui::Action::Reset) {
            let receipts = std::mem::take(&mut self.purchase_form.receipts);
            let reviews = std::mem::take(&mut self.purchase_form.reviews);
            self.purchase_form = Default::default();
            self.purchase_form.receipts = receipts;
            self.purchase_form.reviews = reviews;
            return;
        }
        if let crate::purchase_ui::Action::SelectCancel(index) = action {
            if self.purchase_form.dirty || self.purchase_form.quote.is_some() {
                return;
            }
            if let Some(receipt) = self.purchase_form.receipts.get(index).cloned() {
                self.purchase_form.cancel_purchase = Some(receipt);
                self.purchase_form.dirty = true;
            }
            return;
        }
        if self.edit != self.saved_edit
            || self.sheet_edit != self.saved_sheet
            || self.xp_form != self.xp_saved_form
        {
            self.purchase_form.message=Some(self.t("Save or discard other sheet/XP drafts before reviewing or committing a purchase.","Salve ou descarte outras edições de ficha/XP antes de revisar ou confirmar uma compra.").into());
            return;
        }
        let (Some(lib), Some(id)) = (&mut self.library, &self.purchase_target) else {
            return;
        };
        match action {
            crate::purchase_ui::Action::AddMixed => {
                if let Some(q) = self.purchase_form.quote.clone()
                    && self.queue_mixed_request(
                        q.record_id.clone(),
                        grapevine_workbench::mixed_advancement::Request::Power(Box::new(
                            q.request.clone(),
                        )),
                        q.record_hash,
                    )
                {
                    self.purchase_form.quote = None;
                    self.purchase_form.dirty = false;
                    self.purchase_open = false;
                    self.purchase_form.input.id = uuid::Uuid::new_v4().to_string();
                    self.purchase_form.input.standard_cost_confirmed = false;
                    self.purchase_form.input.approval_reference.clear();
                    self.purchase_form.input.teacher_evidence.clear();
                }
            }
            crate::purchase_ui::Action::SavePlanDraft => {
                match lib.save_advancement_draft(
                    id,
                    &self.purchase_form.plan,
                    lib.revision().unwrap_or(-1),
                    Role::Storyteller,
                ) {
                    Ok(draft_id) => {
                        self.revision = lib.revision().ok();
                        self.purchase_form.dirty = false;
                        self.clear_purchases();
                        self.route = WorkArea::Builder;
                        self.refresh_summaries(false);
                        self.select_record(draft_id);
                        self.open_advancement_draft();
                    }
                    Err(error) => self.purchase_form.message = Some(error.message),
                }
            }
            crate::purchase_ui::Action::AddToPlan => {
                let mut requests = self.purchase_form.plan.clone();
                requests.push(self.purchase_form.input.clone());
                match lib.quote_purchase_batch(
                    id,
                    &requests,
                    lib.revision().unwrap_or(-1),
                    Role::Storyteller,
                ) {
                    Ok(quote) => {
                        self.purchase_form.plan = requests;
                        self.purchase_form.plan_quote = Some(quote);
                        self.purchase_form.quote = None;
                        let input = &mut self.purchase_form.input;
                        input.id = uuid::Uuid::new_v4().to_string();
                        input.power_id.clear();
                        input.relation = None;
                        input.reason.clear();
                        input.affiliation_name.clear();
                        input.affiliation_evidence.clear();
                        input.approval_reference.clear();
                        input.eligibility_evidence.clear();
                        input.teacher_rule = None;
                        input.teacher_evidence.clear();
                        input.standard_cost_confirmed = false;
                        self.purchase_form.dirty = true;
                        self.purchase_form.message = Some(self.t("Purchase queued. Enter fresh power-specific declarations for the next purchase. Nothing has been spent.", "Compra na fila. Informe novas declarações específicas para a próxima compra. Nenhum XP foi gasto.").into());
                    }
                    Err(e) => self.purchase_form.message = Some(e.message),
                }
            }
            crate::purchase_ui::Action::RemoveFromPlan(index) => {
                if index < self.purchase_form.plan.len() {
                    self.purchase_form.plan.remove(index);
                    self.purchase_form.plan_quote = None;
                    self.purchase_form.dirty = true;
                }
            }
            crate::purchase_ui::Action::ReviewPlan => {
                self.purchase_form.plan_quote = None;
                match lib.quote_purchase_batch(
                    id,
                    &self.purchase_form.plan,
                    lib.revision().unwrap_or(-1),
                    Role::Storyteller,
                ) {
                    Ok(quote) => {
                        self.purchase_form.plan_quote = Some(quote);
                        self.purchase_form.message = None;
                    }
                    Err(e) => self.purchase_form.message = Some(e.message),
                }
            }
            crate::purchase_ui::Action::CommitPlan => {
                let Some(quote) = self.purchase_form.plan_quote.clone() else {
                    return;
                };
                if !quote
                    .steps
                    .iter()
                    .map(|step| &step.request)
                    .eq(self.purchase_form.plan.iter())
                {
                    self.purchase_form.plan_quote = None;
                    return;
                }
                match lib.commit_purchase_batch(&quote, Role::Storyteller) {
                    Ok(saved) => {
                        let count = saved.receipts.len();
                        self.revision = lib.revision().ok();
                        self.purchase_form.dirty = false;
                        self.xp_window_open = false;
                        self.xp_view = None;
                        self.xp_revision = None;
                        self.clear_purchases();
                        self.refresh_summaries(true);
                        self.open_purchases();
                        self.purchase_form.message = Some(format!(
                            "{}: {count}",
                            self.t(
                                "Advancement purchases saved together",
                                "Compras de evolução salvas juntas"
                            )
                        ));
                    }
                    Err(e) => self.purchase_form.message = Some(e.message),
                }
            }
            crate::purchase_ui::Action::SaveReview => {
                let result = (|| -> grapevine_workbench::model::Result<()> {
                    let revision = lib.revision()?;
                    let binding = lib.save_purchase_review(
                        id,
                        &self.purchase_form.input,
                        &self.purchase_form.policy_name,
                        revision,
                        Role::Storyteller,
                    )?;
                    self.purchase_form.input = lib.reuse_purchase_review(
                        id,
                        &binding.id,
                        &self.purchase_form.input,
                        revision + 1,
                        Role::Storyteller,
                    )?;
                    self.purchase_form.reviews = lib.purchase_reviews(id, Role::Storyteller)?;
                    Ok(())
                })();
                self.revision = lib.revision().ok();
                self.purchase_form.quote = None;
                self.purchase_form.dirty = true;
                self.purchase_form.message = Some(match result {
                    Ok(()) => self.t("Local review saved and linked. Enter fresh purchase approval and teacher evidence.", "Revisão local salva e vinculada. Informe nova aprovação da compra e evidência do professor.").into(),
                    Err(e) => e.message,
                });
            }
            crate::purchase_ui::Action::ReuseReview(index) => {
                let Some((binding, _)) = self.purchase_form.reviews.get(index) else {
                    return;
                };
                match lib.reuse_purchase_review(
                    id,
                    &binding.id,
                    &self.purchase_form.input,
                    lib.revision().unwrap_or(-1),
                    Role::Storyteller,
                ) {
                    Ok(input) => {
                        self.purchase_form.input = input;
                        self.purchase_form.policy_name = binding.policy.name.clone();
                        self.purchase_form.quote = None;
                        self.purchase_form.dirty = true;
                        self.purchase_form.message = Some(self.t("Review reused. Enter fresh purchase approval and teacher evidence.", "Revisão reutilizada. Informe nova aprovação da compra e evidência do professor.").into());
                    }
                    Err(e) => self.purchase_form.message = Some(e.message),
                }
            }
            crate::purchase_ui::Action::Quote => {
                let rev = match lib.revision() {
                    Ok(r) => r,
                    Err(e) => {
                        self.log_error("purchase.revision", &e);
                        return;
                    }
                };
                match lib.quote_purchase(id, &self.purchase_form.input, rev, Role::Storyteller) {
                    Ok(q) => {
                        self.purchase_form.quote = Some(q);
                        self.purchase_form.dirty = true;
                        self.purchase_form.message = None;
                    }
                    Err(e) => {
                        self.purchase_form.quote = None;
                        self.purchase_form.message = Some(e.message);
                    }
                }
            }
            crate::purchase_ui::Action::Commit => {
                if !self.purchase_form.plan.is_empty() {
                    return;
                }
                let Some(q) = self.purchase_form.quote.clone() else {
                    return;
                };
                if q.request != self.purchase_form.input {
                    self.purchase_form.quote = None;
                    return;
                }
                match lib.commit_purchase(&q, Role::Storyteller) {
                    Ok(receipt) => {
                        let message = format!(
                            "{}: {} · {} XP",
                            self.t("Saved purchase receipt", "Recibo de compra salvo"),
                            receipt.quote.price.name,
                            receipt.quote.price.price.total
                        );
                        self.revision = self.library.as_ref().and_then(|l| l.revision().ok());
                        self.purchase_form.dirty = false;
                        self.xp_window_open = false;
                        self.xp_view = None;
                        self.xp_revision = None;
                        self.clear_purchases();
                        self.refresh_summaries(true);
                        self.open_purchases();
                        self.purchase_form.message = Some(message);
                    }
                    Err(e) => {
                        self.purchase_form.message = Some(e.message);
                    }
                }
            }
            crate::purchase_ui::Action::QuoteCancel => {
                let Some(p) = &self.purchase_form.cancel_purchase else {
                    return;
                };
                let rev = match lib.revision() {
                    Ok(r) => r,
                    Err(e) => {
                        self.purchase_form.message = Some(e.message);
                        return;
                    }
                };
                match lib.quote_cancellation(
                    &p.quote.request.id,
                    &self.purchase_form.cancel_input,
                    rev,
                    Role::Storyteller,
                ) {
                    Ok(q) => {
                        self.purchase_form.cancel_quote = Some(q);
                        self.purchase_form.message = None;
                    }
                    Err(e) => {
                        self.purchase_form.cancel_quote = None;
                        self.purchase_form.message = Some(e.message);
                    }
                }
            }
            crate::purchase_ui::Action::CommitCancel => {
                let Some(q) = self.purchase_form.cancel_quote.clone() else {
                    return;
                };
                if q.request != self.purchase_form.cancel_input {
                    self.purchase_form.cancel_quote = None;
                    return;
                }
                match lib.commit_cancellation(&q, Role::Storyteller) {
                    Ok(saved) => {
                        let message = format!(
                            "{}: {} XP",
                            self.t(
                                "Purchase cancelled; XP refunded",
                                "Compra cancelada; XP reembolsado"
                            ),
                            saved.quote.refund
                        );
                        self.revision = self.library.as_ref().and_then(|l| l.revision().ok());
                        self.xp_window_open = false;
                        self.xp_view = None;
                        self.xp_revision = None;
                        self.clear_purchases();
                        self.refresh_summaries(true);
                        self.open_purchases();
                        self.purchase_form.message = Some(message);
                    }
                    Err(e) => {
                        self.purchase_form.message = Some(e.message);
                    }
                }
            }
            crate::purchase_ui::Action::SelectCancel(_)
            | crate::purchase_ui::Action::Reset
            | crate::purchase_ui::Action::SavePolicy
            | crate::purchase_ui::Action::LoadPolicy => {}
        }
    }
    fn render_purchases(&mut self, ctx: &egui::Context) {
        if self.route != WorkArea::Storyteller || !self.purchase_open {
            return;
        }
        let Some(id) = self.purchase_target.as_ref() else {
            return;
        };
        if self.selected_id.as_ref() != Some(id) {
            self.clear_purchases();
            return;
        }
        let revision = self
            .library
            .as_ref()
            .and_then(|l| l.revision().ok())
            .unwrap_or(-1);
        let mut open = true;
        let mut action = None;
        egui::Window::new(self.t("Purchase / receipts", "Compra / recibos"))
            .id(egui::Id::unique("purchase_window"))
            .default_size([680.0, 590.0])
            .open(&mut open)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("purchase_scroll")
                    .show(ui, |ui| {
                        action = crate::purchase_ui::render(
                            ui,
                            &mut self.purchase_form,
                            revision,
                            self.language,
                        );
                    });
            });
        self.purchase_open = open;
        if let Some(action) = action {
            self.purchase_action(action);
        }
    }

    fn clear_scenarios(&mut self) {
        self.scenario_open = false;
        self.scenario_form = Default::default();
        self.scenario_baseline = None;
        self.scenario_balance = None;
        self.scenario_revision = None;
    }
    fn open_scenarios(&mut self) {
        if self.route.role() != Some(Role::Storyteller) {
            return;
        }
        let (Some(lib), Some(id)) = (&self.library, &self.selected_id) else {
            return;
        };
        let result = (|| -> grapevine_workbench::model::Result<_> {
            let rev = lib.revision()?;
            let record = lib.record(id, Role::Storyteller)?;
            let xp = lib.xp_view(id, Role::Storyteller)?;
            if lib.revision()? != rev {
                return Err(grapevine_workbench::model::error(
                    "STALE_SCENARIO",
                    "Library changed while loading the scenario. Reload and try again.",
                ));
            }
            Ok((rev, record, xp.unspent))
        })();
        match result {
            Ok((rev, record, balance)) => {
                if self
                    .scenario_baseline
                    .as_ref()
                    .is_none_or(|r| r.id != record.id)
                {
                    self.scenario_form = Default::default();
                }
                self.scenario_revision = Some(rev);
                self.scenario_baseline = Some(record);
                self.scenario_balance = balance;
                self.scenario_open = true;
            }
            Err(e) => self.log_error("scenario.open", &e),
        }
    }
    fn scenario_is_stale(&self) -> bool {
        self.library.as_ref().and_then(|l| l.revision().ok()) != self.scenario_revision
    }
    fn render_scenarios(&mut self, ctx: &egui::Context) {
        if self.route.role() != Some(Role::Storyteller) || !self.scenario_open {
            return;
        }
        let Some(record) = self.scenario_baseline.clone() else {
            return;
        };
        let pt = self.language == Language::Portuguese;
        let stale = self.scenario_is_stale();
        let existing_names: Vec<String> =
            grapevine_workbench::creature_profiles::profile(&record.kind)
                .into_iter()
                .flat_map(|p| {
                    [
                        p.primary_power_list.as_str(),
                        p.secondary_power_list.as_str(),
                    ]
                })
                .filter_map(|field| {
                    record
                        .fields
                        .get(field)
                        .and_then(|list| list["Traits"].as_array())
                })
                .flatten()
                .filter_map(|row| row["Name"].as_str().map(str::to_owned))
                .collect();
        let mut open = true;
        let mut reload = false;
        egui::Window::new(if pt {
            "Simular gastos de XP"
        } else {
            "What-if XP spending"
        })
        .id(egui::Id::unique("scenario_window"))
        .default_size([650.0, 570.0])
        .open(&mut open)
        .show(ctx, |ui| {
            ui.heading(&record.name);
            ui.small(format!(
                "{} {}",
                if pt {
                    "Revisão salva"
                } else {
                    "Committed revision"
                },
                self.scenario_revision.unwrap_or_default()
            ));
            if record.build.is_some() {
                ui.label(if pt {
                    "Cópia de criação/teste; não pode gastar XP real."
                } else {
                    "Draft/test copy; it cannot spend live XP."
                });
            }
            if stale {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    if pt {
                        "A biblioteca mudou. Os valores abaixo usam a revisão anterior."
                    } else {
                        "Library changed. The values below use the earlier committed revision."
                    },
                );
            }
            if ui
                .button(if pt {
                    "Recarregar base e recalcular comparações"
                } else {
                    "Reload baseline and recalculate comparisons"
                })
                .clicked()
            {
                reload = true;
            }
            egui::ScrollArea::vertical()
                .id_salt("scenario_scroll")
                .show(ui, |ui| {
                    ui.add_enabled_ui(!stale, |ui| {
                        crate::scenario_ui::render(
                            ui,
                            &mut self.scenario_form,
                            &record.kind,
                            self.scenario_balance.as_deref(),
                            self.language,
                            &existing_names,
                        )
                    });
                });
        });
        self.scenario_open = open;
        if reload {
            self.open_scenarios();
        }
    }

    fn render_xp_window(&mut self, ctx: &egui::Context) {
        if self.route.role() != Some(Role::Storyteller) {
            return;
        }
        if !self.xp_window_open {
            return;
        }
        let Some(view) = self.xp_view.clone() else {
            return;
        };
        let mut open = true;
        let mut action = None;
        let mut refresh = false;
        egui::Window::new(self.t("XP ledger and audit", "Livro de XP e auditoria"))
            .id(egui::Id::unique("xp_window"))
            .default_size([760.0, 620.0])
            .open(&mut open)
            .show(ctx, |ui| {
                if ui
                    .button(self.t("Reload committed ledger", "Recarregar livro salvo"))
                    .clicked()
                {
                    refresh = true;
                }
                if let Some(message) = &self.xp_message {
                    ui.label(message);
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    action = crate::xp_ui::render(ui, &view, &mut self.xp_form, self.language);
                });
            });
        // Closing the floating view retains its unposted form; navigation guards still protect it.
        self.xp_window_open = open;
        if refresh {
            self.request_action(PendingAction::RefreshXp);
        } else if let Some(action) = action {
            self.xp_action(action);
        }
    }
    fn render_xp_roster(&mut self, ctx: &egui::Context) {
        if self.route.role() != Some(Role::Storyteller) {
            return;
        }
        let Some(views) = self.xp_roster.clone() else {
            return;
        };
        let mut open = true;
        let mut target = None;
        let mut export = false;
        egui::Window::new(self.t("Roster XP review","Revisão de XP do elenco")).default_size([760.0,540.0]).open(&mut open).show(ctx,|ui|{
            ui.label(self.t("Findings are review flags. Missing history or authorization remains unverified.","Alertas exigem revisão. Histórico ou autorização ausentes continuam não verificados."));
            if ui.button(self.t("Export review summary…","Exportar resumo da revisão…")).clicked(){export=true;}
            egui::ScrollArea::vertical().show(ui,|ui|{for view in &views {ui.horizontal_wrapped(|ui|{
                let discrepancies=view.findings.iter().filter(|f|f.severity=="discrepancy").count();
                ui.label(format!("{} · {}: {discrepancies} · {}: {}",view.record_name,self.t("Discrepancies","Discrepâncias"),self.t("Other findings","Outros alertas"),view.findings.len()-discrepancies));
                if ui.button(self.t("Open ledger / audit","Abrir XP / auditoria")).clicked(){target=Some(view.record_id.clone());}
            });}});
        });
        if !open {
            self.xp_roster = None;
        }
        if export {
            self.export_xp_audit(None);
        }
        if let Some(id) = target {
            self.request_open_xp(id);
        }
    }
    fn export_xp_audit(&mut self, id: Option<&str>) {
        if self.route.role() != Some(Role::Storyteller) {
            return;
        }
        let Some(library) = &self.library else {
            return;
        };
        let result = library.xp_audit_export(id, Role::Storyteller);
        match result {
            Ok(value) => {
                let Some(path) = FileDialog::new()
                    .set_title(self.t(
                        "Save local XP audit summary",
                        "Salvar resumo local da auditoria XP",
                    ))
                    .set_file_name("Inconnu-XP-Audit.json")
                    .add_filter("Audit summary", &["json"])
                    .save_file()
                else {
                    return;
                };
                let write = (|| -> std::io::Result<()> {
                    use std::io::Write;
                    let mut file = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(path)?;
                    file.write_all(
                        &serde_json::to_vec_pretty(&value).map_err(std::io::Error::other)?,
                    )?;
                    file.sync_all()
                })();
                match write {
                    Ok(()) => {
                        self.xp_message = Some(
                            self.t(
                                "Audit summary saved; private raw history omitted.",
                                "Resumo salvo; histórico privado omitido.",
                            )
                            .into(),
                        )
                    }
                    Err(error) => self.log_error("xp_export", &error.into()),
                }
            }
            Err(error) => self.log_error("xp_export", &error),
        }
    }

    fn render_restore_confirmation(&mut self, ctx: &egui::Context) {
        let Some(path) = self.restore_candidate.clone() else {
            return;
        };
        let mut close = false;
        egui::Window::new(self.t("Confirm restore", "Confirmar restauração"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(self.t(
                    "Restoring replaces the current local library contents.",
                    "A restauração substitui o conteúdo atual da biblioteca local.",
                ));
                ui.label(path.display().to_string());
                ui.horizontal(|ui| {
                    if ui
                        .button(self.t("Restore backup", "Restaurar cópia"))
                        .clicked()
                    {
                        close = true;
                        self.restore_library();
                    }
                    if ui.button(self.t("Cancel", "Cancelar")).clicked() {
                        close = true;
                    }
                });
            });
        if close && self.restore_candidate.is_some() {
            self.restore_candidate = None;
        }
    }

    fn render_pending_confirmation(&mut self, ctx: &egui::Context) {
        if self.pending_action.is_none() {
            return;
        }
        let mut action = None;
        let mut cancel = false;
        egui::Window::new(self.t("Unsaved changes", "Alterações não salvas"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(self.t(
                    "Continuing will discard unposted forms and edit drafts. Save or post them first, or discard to continue.",
                    "Continuar descartará o rascunho atual. Salve ou descarte o rascunho para continuar.",
                ));
                ui.horizontal(|ui| {
                    if ui.button(self.t("Discard and continue", "Descartar e continuar")).clicked() {
                        action = self.pending_action.take();
                    }
                    if ui.button(self.t("Cancel", "Cancelar")).clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel {
            self.pending_action = None;
        }
        if let Some(action) = action {
            self.edit = self.saved_edit.clone();
            self.sheet_edit = self.saved_sheet.clone();
            self.xp_form = self.xp_saved_form.clone();
            self.clear_purchases();
            self.perform_action(action);
        }
    }

    fn render_messages(&mut self, ui: &mut egui::Ui) {
        if let Some(error) = self.error.as_ref() {
            ui.colored_label(ui.visuals().error_fg_color, error);
        } else if let Some(status) = self.status.as_ref() {
            ui.label(status);
        }
    }
}

impl eframe::App for InconnuApp {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        if self.smoke.is_some() {
            input.focused = true;
            // Native screenshot callbacks are separate from RawInput in egui 0.37.
            // Test windows accept only the events queued by the smoke driver.
            input.events.clear();
            input.hovered_files.clear();
            input.dropped_files.clear();
        }
        input.events.append(&mut self.pending_raw_events);
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested())
            && self.has_unsaved_edit()
            && !self.close_after_confirmation
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending_action = Some(PendingAction::Close);
        }
        if self.close_after_confirmation {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().set_visuals(egui::Visuals::dark());
        let ctx = ui.ctx().clone();
        egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            self.render_header(ui);
            ui.separator();
            self.render_routes(ui);
            ui.separator();
            self.render_workspace(ui);
            ui.add_space(4.0);
            self.render_messages(ui);
        });
        self.render_points(&ctx);
        self.render_world(&ctx);
        self.render_plots(&ctx);
        self.render_story(&ctx);
        self.render_rumor_audience(&ctx);
        self.render_associations(&ctx);
        self.render_operations(&ctx);
        self.render_profiles(&ctx);
        self.render_chronicle(&ctx);
        self.render_legacy_profile(&ctx);
        self.render_legacy_calendar(&ctx);
        self.render_support(&ctx);
        self.render_boons(&ctx);
        self.render_searches(&ctx);
        self.render_sessions(&ctx);
        self.render_print(&ctx);
        self.render_playtime(&ctx);
        self.render_purchases(&ctx);
        self.render_published_sources(&ctx);
        self.render_trait_advancement(&ctx);
        self.render_vampire_creation(&ctx);
        self.render_met_reference(&ctx);
        self.render_character_lifecycle(&ctx);
        self.render_mixed_advancement(&ctx);
        self.render_advancement_draft(&ctx);
        self.render_scenarios(&ctx);
        self.render_xp_window(&ctx);
        self.render_xp_roster(&ctx);
        self.render_restore_confirmation(&ctx);
        self.render_pending_confirmation(&ctx);
        self.drive_smoke(&ctx);
    }
}

impl InconnuApp {
    pub fn new_smoke(library_path: PathBuf, output_dir: PathBuf) -> Self {
        let mut app = Self::new(Some(library_path.clone()));
        app.recent_libraries = grapevine_workbench::recent_libraries::RecentLibraries::load(
            output_dir.join("recent-libraries.json"),
        );
        app.recent_libraries.remember(&library_path);
        let source = output_dir.join("smoke-source.gex");
        if let Some(library) = app.library.as_ref() {
            match library.preview_import(&source, "windows-1252", Role::Storyteller) {
                Ok(preview) => app.import_preview = Some(preview),
                Err(error) => app.log_error("ui_smoke.preview", &error),
            }
        }
        app.smoke = Some(SmokeState {
            output_dir,
            library_path,
            phase: SmokePhase::InitialPlayerFrame,
            frames: 0,
            started: Instant::now(),
            phase_started: Instant::now(),
            logged_phase: None,
            next_step: Instant::now(),
            failure: None,
            checks: serde_json::Map::new(),
            screenshot_state: Arc::new(Mutex::new(None)),
            scenario_before: None,
            batch_previous_selection: None,
        });
        app
    }

    fn drive_smoke(&mut self, ctx: &egui::Context) {
        let unsaved_edit = self.has_unsaved_edit();
        let Some(smoke) = self.smoke.as_mut() else {
            return;
        };
        smoke.frames += 1;
        if smoke.logged_phase != Some(smoke.phase) {
            use std::io::Write;
            if let Ok(mut log) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(smoke.output_dir.join("native-smoke-phases.txt"))
            {
                let _ = writeln!(
                    log,
                    "{:.3}s {:?}",
                    smoke.started.elapsed().as_secs_f64(),
                    smoke.phase
                );
            }
            smoke.logged_phase = Some(smoke.phase);
            smoke.phase_started = Instant::now();
        }
        if smoke.phase_started.elapsed() > Duration::from_secs(12)
            && smoke.phase != SmokePhase::AwaitCapture
        {
            smoke.failure = Some(format!(
                "Native UI stalled in {:?}; retained phase timings and screenshot",
                smoke.phase
            ));
            smoke.phase = SmokePhase::Capture;
        }
        if smoke.started.elapsed() > Duration::from_secs(240)
            && smoke.phase != SmokePhase::AwaitCapture
        {
            smoke.failure = Some("UI smoke exceeded its scenario limit before capture".to_owned());
            smoke.phase = SmokePhase::Capture;
        }

        // Keep capture cleanup after the 240-second scenario deadline. Each phase
        // still has its short progress deadline; the expanded suite remains bounded.
        if smoke.started.elapsed() > Duration::from_secs(250) {
            smoke.failure = Some("Native screenshot or UI exceeded its bounded frame limit".into());
            self.finish_smoke(ctx);
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(40));
        // Let the previous event render and settle layout before targeting the next control.
        if Instant::now() < smoke.next_step && smoke.phase != SmokePhase::AwaitCapture {
            return;
        }
        smoke.next_step = Instant::now() + Duration::from_millis(200);
        // Synthetic test phase only: retain a breadcrumb even if a later frame hangs.
        let _ = std::fs::write(
            smoke.output_dir.join("native-smoke-phase.txt"),
            format!("{:?}", smoke.phase),
        );
        match smoke.phase {
            SmokePhase::InitialPlayerFrame => {
                smoke.checks.insert(
                    "player_route_rendered".into(),
                    serde_json::json!(self.route == WorkArea::Player),
                );
                smoke.phase = SmokePhase::RecentMenu;
                ctx.request_repaint_after(Duration::from_millis(40));
            }
            SmokePhase::RecentMenu => {
                if let Some(rect) = probe(ctx, "recent_menu") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::RecentOpen;
                }
            }
            SmokePhase::RecentOpen => {
                if let Some(rect) = probe(ctx, "recent_first") {
                    let path = smoke.output_dir.join("recent-libraries-menu-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    self.status = None;
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::RecentOpened;
                }
            }
            SmokePhase::RecentOpened => {
                if self.status.is_some() {
                    let opened = self
                        .library
                        .as_ref()
                        .is_some_and(|lib| lib.path == smoke.library_path)
                        && self.error.is_none();
                    let saved = grapevine_workbench::recent_libraries::RecentLibraries::load(
                        smoke.output_dir.join("recent-libraries.json"),
                    );
                    let persisted = saved.paths().first() == Some(&smoke.library_path)
                        && saved.warning().is_none();
                    smoke.checks.insert(
                        "recent_native_open_clicked".into(),
                        serde_json::json!(opened),
                    );
                    smoke.checks.insert(
                        "recent_native_isolated_restart".into(),
                        serde_json::json!(persisted),
                    );
                    if opened && persisted {
                        // Reopening intentionally clears import previews. Prepare the next
                        // synthetic import only after the actual recent-open check succeeds.
                        match self.library.as_ref().unwrap().preview_import(
                            &smoke.output_dir.join("smoke-source.gex"),
                            "windows-1252",
                            Role::Storyteller,
                        ) {
                            Ok(preview) => {
                                self.import_preview = Some(preview);
                                smoke.phase = SmokePhase::ClickStoryteller;
                            }
                            Err(_) => {
                                smoke.failure = Some(
                                    "Could not prepare synthetic import after recent reopen".into(),
                                );
                                smoke.phase = SmokePhase::Capture;
                            }
                        }
                    } else {
                        smoke.failure =
                            Some("Recent library click or isolated persistence failed".into());
                        smoke.phase = SmokePhase::Capture;
                    }
                }
            }
            SmokePhase::ClickStoryteller => {
                if let Some(rect) = self.smoke_controls.storyteller {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke
                        .checks
                        .insert("storyteller_route_clicked".into(), serde_json::json!(true));
                    smoke.phase = SmokePhase::ClickImportCommit;
                } else {
                    ctx.request_repaint_after(Duration::from_millis(40));
                }
            }
            SmokePhase::ClickImportCommit => {
                if let Some(rect) = self.smoke_controls.commit_import {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.checks.insert(
                        "import_commit_button_clicked".into(),
                        serde_json::json!(true),
                    );
                    smoke.phase = SmokePhase::ClickName;
                } else {
                    smoke.failure =
                        Some("No import preview commit control was available".to_owned());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::ClickName => {
                if let Some(rect) = self.smoke_controls.name {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke
                        .checks
                        .insert("name_field_clicked".into(), serde_json::json!(true));
                    smoke.phase = SmokePhase::TypeName;
                } else {
                    scroll_detail_to(&mut self.pending_raw_events, ctx, "sheet_name_bounds");
                }
            }
            SmokePhase::TypeName => {
                if let Some(rect) = self.smoke_controls.name {
                    inject_replace_text(
                        &mut self.pending_raw_events,
                        rect,
                        "Smoke Edited Character",
                    );
                    smoke.phase = SmokePhase::ScrollClan;
                } else {
                    ctx.request_repaint_after(Duration::from_millis(40));
                }
            }
            SmokePhase::ScrollClan => {
                if let Some(rect) = probe(ctx, "sheet_clan_probe") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::TypeClan;
                } else {
                    scroll_detail_to(&mut self.pending_raw_events, ctx, "sheet_clan_bounds");
                }
            }
            SmokePhase::TypeClan => {
                if let Some(rect) = probe(ctx, "sheet_clan_probe") {
                    inject_replace_text(
                        &mut self.pending_raw_events,
                        rect,
                        "Native Clan [ST]NATIVE_PRIVATE_CLAN[/ST]",
                    );
                    smoke.phase = SmokePhase::CheckClan;
                }
            }
            SmokePhase::CheckClan => {
                if self
                    .sheet_edit
                    .as_ref()
                    .and_then(|edit| edit.identity.get("Clan"))
                    .map(String::as_str)
                    == Some("Native Clan [ST]NATIVE_PRIVATE_CLAN[/ST]")
                {
                    let screenshot = smoke.output_dir.join("identity-edit-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&screenshot, &image);
                    });
                    smoke.phase = SmokePhase::ClickPortuguese;
                } else {
                    smoke.failure =
                        Some("Native identity text input did not update the sheet".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::ClickPortuguese => {
                if self.edit.name != "Smoke Edited Character" {
                    smoke.failure = Some(format!(
                        "Injected egui text event did not update the name field: {:?}",
                        self.edit.name
                    ));
                    smoke.phase = SmokePhase::Capture;
                } else if let Some(rect) = self.smoke_controls.portuguese {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke
                        .checks
                        .insert("text_input_event".into(), serde_json::json!(true));
                    smoke.phase = SmokePhase::ClickSave;
                } else {
                    ctx.request_repaint_after(Duration::from_millis(40));
                }
            }
            SmokePhase::ClickSave => {
                smoke.checks.insert(
                    "native_identity_text_preserved_across_language".into(),
                    serde_json::json!(
                        self.sheet_edit
                            .as_ref()
                            .and_then(|edit| edit.identity.get("Clan"))
                            .map(String::as_str)
                            == Some("Native Clan [ST]NATIVE_PRIVATE_CLAN[/ST]")
                    ),
                );
                if self.language != Language::Portuguese {
                    smoke.failure =
                        Some("Portuguese language button event was not applied".to_owned());
                    smoke.phase = SmokePhase::Capture;
                } else if self.edit.name != "Smoke Edited Character" {
                    smoke.failure =
                        Some("Changing the language lost the unsaved name edit".to_owned());
                    smoke.phase = SmokePhase::Capture;
                } else if let Some(rect) = self.smoke_controls.save {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke
                        .checks
                        .insert("portuguese_button_event".into(), serde_json::json!(true));
                    smoke.checks.insert(
                        "unsaved_edit_preserved_on_language_change".into(),
                        serde_json::json!(true),
                    );
                    smoke.phase = SmokePhase::ClickBuilder;
                } else {
                    ctx.request_repaint_after(Duration::from_millis(40));
                }
            }
            SmokePhase::ClickBuilder => {
                if unsaved_edit {
                    smoke.failure = Some("Save did not persist".into());
                    smoke.phase = SmokePhase::Capture;
                } else {
                    self.menu_preview=grapevine_workbench::menus::inspect(br#"<grapevinemenus version="3" size="1"><menu name="Arcanoi"><item name="Common Fixture" note="common"/><item name="Guild Fixture" note="guild"/></menu></grapevinemenus>"#,"synthetic-menu.gv3").ok().flatten();
                    self.menu_query.clear();
                    smoke.checks.insert(
                        "menu_preview_loaded".into(),
                        serde_json::json!(
                            self.menu_preview
                                .as_ref()
                                .is_some_and(|m| m.rows.len() == 3)
                        ),
                    );
                    smoke.phase = SmokePhase::MenuType;
                }
            }
            SmokePhase::MenuType => {
                if let Some(rect) = self.smoke_controls.menu_search {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::MenuInput;
                }
            }
            SmokePhase::MenuInput => {
                if let Some(rect) = self.smoke_controls.menu_search {
                    inject_replace_text(&mut self.pending_raw_events, rect, "Guild");
                    smoke.phase = SmokePhase::MenuCheck;
                }
            }
            SmokePhase::MenuCheck => {
                if self.menu_query != "Guild" {
                    smoke.failure = Some("Menu search native typing failed".into());
                    smoke.phase = SmokePhase::Capture;
                } else {
                    smoke
                        .checks
                        .insert("menu_native_search".into(), serde_json::json!(true));
                    let path = smoke.output_dir.join("menu-preview-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::MenuSave;
                }
            }
            SmokePhase::MenuSave => {
                if let Some(rect) = self.smoke_controls.menu_save {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::MenuSaved;
                }
            }
            SmokePhase::MenuSaved => {
                let saved = self
                    .menu_preview
                    .as_ref()
                    .is_some_and(|m| self.active_menu_hash.as_ref() == Some(&m.sha256))
                    && self.menu_sources.len() == 1;
                if saved {
                    smoke
                        .checks
                        .insert("menu_saved_and_selected".into(), serde_json::json!(true));
                    smoke.phase = SmokePhase::MenuClose;
                } else {
                    smoke.failure = Some("Menu save/selection did not persist".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::MenuClose => {
                if let Some(rect) = self.smoke_controls.menu_close {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::MenuResume;
                }
            }
            SmokePhase::MenuResume => {
                if self.menu_preview.is_some() {
                    smoke.failure = Some("Menu dismiss failed".into());
                    smoke.phase = SmokePhase::Capture;
                } else if let Some(rect) = self.smoke_controls.xp {
                    smoke
                        .checks
                        .insert("menu_dismissed".into(), serde_json::json!(true));
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::XpOpen;
                    smoke
                        .checks
                        .insert("save_button_event".into(), serde_json::json!(true));
                }
            }
            SmokePhase::XpOpen => {
                if self.xp_window_open {
                    smoke.phase = SmokePhase::XpInitialize;
                    smoke
                        .checks
                        .insert("xp_window_opened".into(), serde_json::json!(true));
                } else {
                    smoke.failure = Some("XP window did not open".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::XpInitialize => {
                if let Some(rect) = probe(ctx, "xp_init_probe") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::XpReady;
                } else {
                    scroll_xp(&mut self.pending_raw_events, ctx, -200.0);
                }
            }
            SmokePhase::XpReady => {
                if self
                    .xp_view
                    .as_ref()
                    .is_some_and(|v| v.unspent.as_deref() == Some("10"))
                {
                    self.xp_form.reason = "Synthetic award".into();
                    self.xp_form.evidence = "SMOKE_AWARD".into();
                    self.xp_saved_form = self.xp_form.clone();
                    smoke.phase = SmokePhase::XpAmount;
                    smoke
                        .checks
                        .insert("xp_opening_committed".into(), serde_json::json!(true));
                } else {
                    smoke.failure = Some("XP opening was not committed".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::XpAmount => {
                if let Some(rect) = probe(ctx, "xp_amount_probe") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::XpPost;
                } else {
                    scroll_xp(&mut self.pending_raw_events, ctx, -200.0);
                }
            }
            SmokePhase::XpPost => {
                if self.xp_form.amount.is_empty() {
                    if let Some(rect) = probe(ctx, "xp_amount_probe") {
                        inject_replace_text(&mut self.pending_raw_events, rect, "0.25");
                    }
                } else if self.xp_form.amount == "0.25" {
                    if let Some(rect) = probe(ctx, "xp_post_probe") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::XpAwardSaved;
                    } else {
                        scroll_xp(&mut self.pending_raw_events, ctx, -140.0);
                    }
                } else {
                    smoke.failure = Some("XP native amount input differs".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::XpAwardSaved => {
                if self
                    .xp_view
                    .as_ref()
                    .is_some_and(|v| v.unspent.as_deref() == Some("10.25"))
                {
                    self.xp_form.kind = grapevine_workbench::xp::XpKind::Spend;
                    self.xp_form.reason = "Synthetic overspend".into();
                    self.xp_form.evidence = "SMOKE_DENIED".into();
                    self.xp_saved_form = self.xp_form.clone();
                    smoke.phase = SmokePhase::XpNegativeAmount;
                    smoke
                        .checks
                        .insert("xp_fractional_award_saved".into(), serde_json::json!(true));
                } else {
                    smoke.failure = Some("Native XP award was not saved".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::XpNegativeAmount => {
                if let Some(rect) = probe(ctx, "xp_amount_probe") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::XpNegativePost;
                } else {
                    scroll_xp(&mut self.pending_raw_events, ctx, 180.0);
                }
            }
            SmokePhase::XpNegativePost => {
                if self.xp_form.amount.is_empty() {
                    if let Some(rect) = probe(ctx, "xp_amount_probe") {
                        inject_replace_text(&mut self.pending_raw_events, rect, "100");
                    }
                } else if self.xp_form.amount == "100" {
                    if let Some(rect) = probe(ctx, "xp_post_probe") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::XpRejected;
                    } else {
                        scroll_xp(&mut self.pending_raw_events, ctx, -180.0);
                    }
                } else {
                    smoke.failure = Some("Native overspend input differs".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::XpRejected => {
                let rejected = self
                    .xp_message
                    .as_ref()
                    .is_some_and(|s| s.contains("XP_NEGATIVE_HISTORY"))
                    && self.xp_view.as_ref().is_some_and(|v| {
                        v.unspent.as_deref() == Some("10.25")
                            && v.ledger.as_ref().is_some_and(|l| l.entries.len() == 1)
                    });
                if rejected {
                    let path = smoke.output_dir.join("xp-audit-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    if let (Some(library), Some(view)) = (&self.library, &self.xp_view)
                        && let Ok(report) =
                            library.xp_audit_export(Some(&view.record_id), Role::Storyteller)
                    {
                        let _ = std::fs::write(
                            smoke.output_dir.join("xp-audit-summary.json"),
                            serde_json::to_vec_pretty(&report).unwrap_or_default(),
                        );
                    }
                    smoke.checks.insert(
                        "xp_negative_spending_rejected".into(),
                        serde_json::json!(true),
                    );
                    smoke.phase = SmokePhase::XpCloseAfterCapture;
                } else {
                    smoke.failure = Some("Overspend was not visibly rejected".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::XpCloseAfterCapture => {
                self.xp_form = Default::default();
                self.xp_saved_form = Default::default();
                self.xp_window_open = false;
                self.error = None;
                smoke.phase = SmokePhase::PlaytimeClick;
            }
            SmokePhase::PlaytimeClick => {
                if let Some(rect) = self.smoke_controls.playtime {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PlaytimeOpen;
                }
            }
            SmokePhase::PlaytimeOpen => {
                if !self.playtime_open {
                    smoke.failure = Some("Playtime review button did not open".into());
                    smoke.phase = SmokePhase::Capture;
                } else {
                    self.playtime_form.input = grapevine_workbench::playtime::Request {
                        start_month: "2026-01".into(),
                        end_month: "2026-01".into(),
                        class: grapevine_workbench::playtime::CapClass::Standard,
                        policy_scope_confirmed: true,
                        complete_regular_history_confirmed: true,
                        full_month_eligible_confirmed: true,
                        home_chronicle: "SMOKE_PRIVATE_CHRONICLE".into(),
                        review_evidence: "SMOKE_PRIVATE_PLAYTIME_EVIDENCE".into(),
                        ..Default::default()
                    };
                    self.playtime_form.classification_reason =
                        "SMOKE_PRIVATE_CLASSIFICATION_REASON".into();
                    self.playtime_form.classification_reference =
                        "SMOKE_PRIVATE_CLASSIFICATION_REFERENCE".into();
                    if let Some(rect) = probe(ctx, "playtime_load_awards") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::PlaytimeLoadAwards;
                    }
                }
            }
            SmokePhase::PlaytimeLoadAwards => {
                if self
                    .playtime_form
                    .awards_current(self.library.as_ref().unwrap().revision().unwrap())
                    && let Some(rect) = probe(ctx, "playtime_calculate")
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PlaytimePending;
                }
            }
            SmokePhase::PlaytimePending => {
                if let Some(value) = &self.playtime_form.report {
                    let good = value["report"]["status"] == "InsufficientEvidence"
                        && value["report"]["months"][0]["unclassified_awarded"] == "0.25"
                        && value["report"]["months"][0]["regular_awarded"] == "0";
                    smoke.checks.insert(
                        "award_review_native_unclassified_insufficient".into(),
                        serde_json::json!(good),
                    );
                    if !good {
                        smoke.failure =
                            Some("Unclassified awards were silently treated as regular".into());
                        smoke.phase = SmokePhase::Capture;
                    } else if let Some(rect) = probe(ctx, "playtime_mark_regular") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::PlaytimeClassified;
                    }
                }
            }
            SmokePhase::PlaytimeClassified => {
                if !self.playtime_form.input.award_reviews.is_empty()
                    && let Some(rect) = probe(ctx, "playtime_calculate")
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PlaytimeCalculated;
                }
            }
            SmokePhase::PlaytimeCalculated => {
                if let Some(value) = &self.playtime_form.report {
                    let good = value["report"]["status"] == "WithinDeclaredCeiling"
                        && value["report"]["months"][0]["opening_earned"] == "10"
                        && value["report"]["months"][0]["regular_awarded"] == "0.25"
                        && value["report"]["months"][0]["ceiling"] == "12"
                        && value["library_revision"].as_i64()
                            == self.library.as_ref().unwrap().revision().ok()
                        && !value.to_string().contains("SMOKE_PRIVATE");
                    smoke.checks.insert(
                        "playtime_native_calculation_read_only_private".into(),
                        serde_json::json!(good),
                    );
                    if !good {
                        smoke.failure =
                            Some("Playtime review had incorrect monthly values or privacy".into());
                        smoke.phase = SmokePhase::Capture;
                    } else {
                        let bytes = serde_json::to_vec_pretty(value).unwrap();
                        let path = smoke.output_dir.join("playtime-transfer-summary.json");
                        let saved = grapevine_workbench::safe_files::write_new(&path, &bytes)
                            .is_ok()
                            && std::fs::read(path).is_ok_and(|v| v == bytes);
                        smoke.checks.insert(
                            "playtime_transfer_summary_saved".into(),
                            serde_json::json!(saved),
                        );
                        self.playtime_form.input.award_reviews[0].class =
                            grapevine_workbench::playtime::AwardClass::SeparateReview;
                        if let Some(rect) = probe(ctx, "playtime_calculate") {
                            inject_click(&mut self.pending_raw_events, rect);
                            smoke.phase = SmokePhase::PlaytimeSeparate;
                        }
                    }
                } else {
                    smoke.failure = Some(format!(
                        "Playtime calculation failed: {:?}",
                        self.playtime_form.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::PlaytimeSeparate => {
                if let Some(value) = &self.playtime_form.report {
                    let good = value["report"]["status"] == "InsufficientEvidence"
                        && value["report"]["months"][0]["separate_review_awarded"] == "0.25"
                        && value["report"]["months"][0]["source_awarded"] == "0.25";
                    smoke.checks.insert(
                        "award_review_native_separate_not_exempt".into(),
                        serde_json::json!(good),
                    );
                    if !good {
                        smoke.failure =
                            Some("Separate-review awards produced a clean exemption".into());
                        smoke.phase = SmokePhase::Capture;
                    } else {
                        self.playtime_form.input.award_reviews[0].class =
                            grapevine_workbench::playtime::AwardClass::Regular;
                        self.playtime_form.input.complete_regular_history_confirmed = false;
                        if let Some(rect) = probe(ctx, "playtime_calculate") {
                            inject_click(&mut self.pending_raw_events, rect);
                            smoke.phase = SmokePhase::PlaytimeUnqualified;
                        }
                    }
                }
            }
            SmokePhase::PlaytimeUnqualified => {
                let good = self
                    .playtime_form
                    .report
                    .as_ref()
                    .is_some_and(|v| v["report"]["status"] == "InsufficientEvidence");
                smoke.checks.insert(
                    "playtime_incomplete_history_unverified".into(),
                    serde_json::json!(good),
                );
                if !good {
                    smoke.failure = Some("Incomplete history was falsely clean".into());
                    smoke.phase = SmokePhase::Capture;
                } else {
                    let path = smoke.output_dir.join("playtime-review-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::PlaytimeCloseAfterCapture;
                }
            }
            SmokePhase::PlaytimeCloseAfterCapture => {
                if smoke.output_dir.join("playtime-review-smoke.bmp").is_file() {
                    let hash = self.playtime_form.report.as_ref().unwrap()["record_hash"]
                        .as_str()
                        .unwrap()
                        .to_owned();
                    ctx.data_mut(|d| {
                        d.insert_temp(
                            egui::Id::unique("private_review_original_record_hash"),
                            hash,
                        )
                    });
                    smoke.phase = SmokePhase::PlaytimeSavePrivate;
                }
            }
            SmokePhase::PlaytimeSavePrivate => {
                if let Some(rect) = probe(ctx, "playtime_save_private") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PlaytimePrivateSaved;
                }
            }
            SmokePhase::PlaytimePrivateSaved => {
                if !self.playtime_form.saved_reviews.is_empty() {
                    let lib = self.library.as_ref().unwrap();
                    let id = self.playtime_target.as_ref().unwrap();
                    let value = lib
                        .playtime_report(
                            id,
                            &self.playtime_form.input,
                            lib.revision().unwrap(),
                            Role::Storyteller,
                        )
                        .unwrap();
                    let original = ctx
                        .data(|d| {
                            d.get_temp::<String>(egui::Id::unique(
                                "private_review_original_record_hash",
                            ))
                        })
                        .unwrap();
                    let good = value["record_hash"].as_str() == Some(original.as_str())
                        && lib
                            .xp_view(id, Role::Storyteller)
                            .unwrap()
                            .unspent
                            .as_deref()
                            == Some("10.25");
                    smoke.checks.insert(
                        "private_review_native_save_preserves_xp_record".into(),
                        serde_json::json!(good),
                    );
                    if !good {
                        smoke.failure =
                            Some("Private review save changed character or XP data".into());
                        smoke.phase = SmokePhase::Capture;
                    } else if let Some(rect) = probe(ctx, "playtime_load_private") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::PlaytimePrivateConfirm;
                    }
                }
            }
            SmokePhase::PlaytimePrivateConfirm => {
                if self.playtime_form.pending_load.is_some()
                    && let Some(rect) = probe(ctx, "playtime_confirm_load")
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PlaytimePrivateLoaded;
                }
            }
            SmokePhase::PlaytimePrivateLoaded => {
                let input = &self.playtime_form.input;
                if input.class == grapevine_workbench::playtime::CapClass::Unknown
                    && input.award_reviews.len() == 1
                {
                    let lib = self.library.as_ref().unwrap();
                    let public =
                        serde_json::to_string(&lib.public_chronicle_profile().unwrap()).unwrap();
                    let good = !input.policy_scope_confirmed
                        && !input.complete_regular_history_confirmed
                        && !input.full_month_eligible_confirmed
                        && input.home_chronicle.is_empty()
                        && input.review_evidence.is_empty()
                        && self.playtime_form.report.is_none()
                        && !public.contains("SMOKE_PRIVATE")
                        && !public.contains(grapevine_workbench::playtime_review::FIELD);
                    smoke.checks.insert(
                        "private_review_native_load_resets_authority_private".into(),
                        serde_json::json!(good),
                    );
                    if !good {
                        smoke.failure=Some("Private review load restored authority assumptions or exposed private evidence".into());
                        smoke.phase = SmokePhase::Capture;
                    } else {
                        let path = smoke.output_dir.join("private-review-loaded-smoke.bmp");
                        ctx.request_screenshot(move |image| {
                            let _ = write_bmp(&path, &image);
                        });
                        smoke.phase = SmokePhase::PlaytimePrivateDone;
                    }
                }
            }
            SmokePhase::PlaytimePrivateDone => {
                if smoke
                    .output_dir
                    .join("private-review-loaded-smoke.bmp")
                    .is_file()
                {
                    self.playtime_open = false;
                    self.playtime_form = Default::default();
                    smoke.phase = SmokePhase::ScenarioClick;
                }
            }
            SmokePhase::ScenarioClick => {
                if let (Some(lib), Some(id), Some(rect)) = (
                    &self.library,
                    &self.selected_id,
                    self.smoke_controls.scenario,
                ) {
                    smoke.scenario_before = lib.revision().ok().zip(
                        lib.record(id, Role::Storyteller)
                            .ok()
                            .and_then(|r| serde_json::to_string(&r).ok()),
                    );
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioOpen;
                }
            }
            SmokePhase::ScenarioOpen => {
                if !self.scenario_open {
                    smoke.failure = Some("Scenario button did not open preview".into());
                    smoke.phase = SmokePhase::Capture;
                } else if let Some(rect) = probe(ctx, "scenario_profile") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioProfile;
                }
            }
            SmokePhase::ScenarioProfile => {
                if self.scenario_form.allocation.profile
                    && let Some(rect) = probe(ctx, "scenario_power0")
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioPower0;
                }
            }
            SmokePhase::ScenarioPower0 => {
                if self
                    .scenario_form
                    .allocation
                    .choices
                    .entry(
                        grapevine_workbench::purchase_preview::AnimalismPower::ALL[0]
                            .stable_id()
                            .into(),
                    )
                    .or_default()
                    .selected
                    && let Some(rect) = probe(ctx, "scenario_in0")
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioIn0;
                }
            }
            SmokePhase::ScenarioIn0 => {
                if self
                    .scenario_form
                    .allocation
                    .choices
                    .entry(
                        grapevine_workbench::purchase_preview::AnimalismPower::ALL[0]
                            .stable_id()
                            .into(),
                    )
                    .or_default()
                    .relation
                    == Some(grapevine_workbench::pricing::ClanRelation::InClan)
                    && let Some(rect) = probe(ctx, "scenario_power1")
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioPower1;
                }
            }
            SmokePhase::ScenarioPower1 => {
                if self
                    .scenario_form
                    .allocation
                    .choices
                    .entry(
                        grapevine_workbench::purchase_preview::AnimalismPower::ALL[1]
                            .stable_id()
                            .into(),
                    )
                    .or_default()
                    .selected
                    && let Some(rect) = probe(ctx, "scenario_out1")
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioOut1;
                }
            }
            SmokePhase::ScenarioOut1 => {
                if self
                    .scenario_form
                    .allocation
                    .choices
                    .entry(
                        grapevine_workbench::purchase_preview::AnimalismPower::ALL[1]
                            .stable_id()
                            .into(),
                    )
                    .or_default()
                    .relation
                    == Some(grapevine_workbench::pricing::ClanRelation::OutOfClan)
                    && let Some(rect) = probe(ctx, "scenario_assumed")
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioAssumed;
                }
            }
            SmokePhase::ScenarioAssumed => {
                if self.scenario_form.allocation.assumed_budget
                    && let Some(rect) = probe(ctx, "scenario_budget")
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioBudgetClick;
                }
            }
            SmokePhase::ScenarioBudgetClick => {
                if let Some(rect) = probe(ctx, "scenario_budget") {
                    inject_replace_text(&mut self.pending_raw_events, rect, "6");
                    smoke.phase = SmokePhase::ScenarioBudgetType;
                }
            }
            SmokePhase::ScenarioBudgetType => {
                if self.scenario_form.allocation.budget == "6" {
                    smoke.phase = SmokePhase::ScenarioCheck;
                }
            }
            SmokePhase::ScenarioCheck => {
                let q = self
                    .scenario_form
                    .allocation
                    .evaluate("Vampire", self.scenario_balance.as_deref());
                let ok = q.is_ok_and(|q| {
                    q.total_spend == "7"
                        && q.projected.as_deref() == Some("-1")
                        && q.shortfall.as_deref() == Some("1")
                        && !q.purchase_enabled
                });
                smoke.checks.insert(
                    "scenario_native_choices_and_shortfall".into(),
                    serde_json::json!(ok),
                );
                if !ok {
                    smoke.failure = Some(
                        "Scenario native choice calculation did not show7XP/1XPshortfall".into(),
                    );
                    smoke.phase = SmokePhase::Capture;
                } else if let Some(rect) = probe(ctx, "scenario_keep_a") {
                    let path = smoke.output_dir.join("spending-preview-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioKeep;
                } else {
                    let rect = ctx.content_rect();
                    self.pending_raw_events
                        .push(egui::Event::PointerMoved(rect.center()));
                    self.pending_raw_events.push(egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -160.0),
                        source: egui::MouseWheelSource::Unknown,
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
            SmokePhase::ScenarioKeep => {
                let kept = self.scenario_form.comparisons[0].as_ref()
                    == Some(&self.scenario_form.allocation);
                smoke.checks.insert(
                    "scenario_comparison_retained".into(),
                    serde_json::json!(kept),
                );
                // Compare an alternative without replacing A; both remain hypothetical.
                self.scenario_form
                    .allocation
                    .choices
                    .entry(
                        grapevine_workbench::purchase_preview::AnimalismPower::ALL[1]
                            .stable_id()
                            .into(),
                    )
                    .or_default()
                    .selected = false;
                self.scenario_form
                    .allocation
                    .choices
                    .entry(
                        grapevine_workbench::purchase_preview::AnimalismPower::ALL[2]
                            .stable_id()
                            .into(),
                    )
                    .or_default()
                    .selected = true;
                self.scenario_form
                    .allocation
                    .choices
                    .entry(
                        grapevine_workbench::purchase_preview::AnimalismPower::ALL[2]
                            .stable_id()
                            .into(),
                    )
                    .or_default()
                    .relation = Some(grapevine_workbench::pricing::ClanRelation::OutOfClan);
                let mixed = self
                    .scenario_form
                    .allocation
                    .evaluate("Vampire", self.scenario_balance.as_deref())
                    .is_ok_and(|q| {
                        q.items.len() == 2
                            && q.total_spend == "7"
                            && q.projected.as_deref() == Some("-1")
                            && q.items[1].power.family() == "Auspex"
                    });
                smoke.checks.insert(
                    "scenario_cross_family_cart".into(),
                    serde_json::json!(mixed),
                );
                self.scenario_form
                    .allocation
                    .choices
                    .entry(
                        grapevine_workbench::purchase_preview::AnimalismPower::ALL[2]
                            .stable_id()
                            .into(),
                    )
                    .or_default()
                    .selected = false;
                let alternative = self
                    .scenario_form
                    .allocation
                    .evaluate("Vampire", self.scenario_balance.as_deref())
                    .is_ok_and(|q| q.total_spend == "3" && q.projected.as_deref() == Some("3"));
                smoke.checks.insert(
                    "scenario_alternative_isolated".into(),
                    serde_json::json!(
                        alternative
                            && self.scenario_form.comparisons[0].as_ref().is_some_and(|a| a
                                .choices
                                .get(
                                    grapevine_workbench::purchase_preview::AnimalismPower::ALL[1]
                                        .stable_id()
                                )
                                .is_some_and(|c| c.selected))
                    ),
                );
                smoke.phase = SmokePhase::ScenarioFamilyClick;
            }
            SmokePhase::ScenarioFamilyClick => {
                if let Some(rect) = probe(ctx, "scenario_family_menu") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioFamilyMenu;
                } else {
                    self.pending_raw_events
                        .push(egui::Event::PointerMoved(ctx.content_rect().center()));
                    self.pending_raw_events.push(egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, 180.0),
                        source: egui::MouseWheelSource::Unknown,
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
            SmokePhase::ScenarioFamilyMenu => {
                if let Some(rect) = probe(ctx, "scenario_family_auspex") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioFamilyChoice;
                }
            }
            SmokePhase::ScenarioFamilyChoice => {
                if self.scenario_form.family == 1
                    && let Some(rect) = probe(ctx, "scenario_power2")
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioFamilyChecked;
                }
            }
            SmokePhase::ScenarioFamilyChecked => {
                self.scenario_form
                    .allocation
                    .choices
                    .entry(
                        grapevine_workbench::purchase_preview::AnimalismPower::ALL[2]
                            .stable_id()
                            .into(),
                    )
                    .or_default()
                    .relation = Some(grapevine_workbench::pricing::ClanRelation::OutOfClan);
                let valid = self.scenario_form.family == 1
                    && self
                        .scenario_form
                        .allocation
                        .choices
                        .entry(
                            grapevine_workbench::purchase_preview::AnimalismPower::ALL[0]
                                .stable_id()
                                .into(),
                        )
                        .or_default()
                        .selected
                    && self
                        .scenario_form
                        .allocation
                        .choices
                        .entry(
                            grapevine_workbench::purchase_preview::AnimalismPower::ALL[2]
                                .stable_id()
                                .into(),
                        )
                        .or_default()
                        .selected
                    && self.scenario_form.comparisons[0].as_ref().is_some_and(|a| {
                        a.choices
                            .get(
                                grapevine_workbench::purchase_preview::AnimalismPower::ALL[1]
                                    .stable_id(),
                            )
                            .is_some_and(|c| c.selected)
                    })
                    && self
                        .scenario_form
                        .allocation
                        .evaluate("Vampire", self.scenario_balance.as_deref())
                        .is_ok_and(|q| q.total_spend == "7");
                smoke.checks.insert(
                    "scenario_family_filter_native_selection".into(),
                    serde_json::json!(valid),
                );
                if !valid {
                    smoke.failure = Some("Changing family lost selections or comparison".into());
                    smoke.phase = SmokePhase::Capture;
                } else {
                    smoke.phase = SmokePhase::ScenarioTierClick;
                }
            }
            SmokePhase::ScenarioTierClick => {
                if let Some(rect) = probe(ctx, "scenario_tier_menu") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioTierMenu;
                } else {
                    scroll_xp(&mut self.pending_raw_events, ctx, 200.0);
                }
            }
            SmokePhase::ScenarioTierMenu => {
                if let Some(rect) = probe(ctx, "scenario_tier_intermediate") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ScenarioTierChecked;
                }
            }
            SmokePhase::ScenarioTierChecked => {
                let index = grapevine_workbench::purchase_preview::AnimalismPower::ALL
                    .iter()
                    .position(|power| {
                        power.stable_id()
                            == "met.lotn-revised.auspex.intermediate.the_spirits_touch"
                    })
                    .unwrap();
                let valid = self.scenario_form.tier == crate::scenario_ui::TierFilter::Intermediate
                    && probe(ctx, &format!("scenario_power{index}")).is_some()
                    && self
                        .scenario_form
                        .allocation
                        .evaluate("Vampire", self.scenario_balance.as_deref())
                        .is_ok_and(|q| q.total_spend == "7")
                    && self.scenario_form.comparisons[0].is_some();
                smoke.checks.insert(
                    "scenario_tier_filter_retains_cart_native".into(),
                    serde_json::json!(valid),
                );
                if valid {
                    let path = smoke.output_dir.join("tier-filter-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::ScenarioFinish;
                } else {
                    smoke.failure =
                        Some("Changing tier hid new entries or lost the combined cart".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::ScenarioFinish => {
                let after = self.library.as_ref().and_then(|lib| {
                    self.selected_id.as_ref().and_then(|id| {
                        lib.revision().ok().zip(
                            lib.record(id, Role::Storyteller)
                                .ok()
                                .and_then(|r| serde_json::to_string(&r).ok()),
                        )
                    })
                });
                let unchanged = after.is_some() && after == smoke.scenario_before;
                smoke.checks.insert(
                    "scenario_live_sheet_revision_unchanged".into(),
                    serde_json::json!(unchanged),
                );
                if !unchanged {
                    smoke.failure = Some("Scenario unexpectedly changed committed data".into());
                }
                self.scenario_open = false;
                smoke.phase = SmokePhase::PurchaseClick;
            }
            SmokePhase::PurchaseClick => {
                if let Some(rect) = self.smoke_controls.purchase {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PurchaseOpen;
                }
            }
            SmokePhase::PurchaseOpen => {
                if self.purchase_open {
                    use grapevine_workbench::{
                        pricing::{ClanRelation, PricingProfile},
                        purchase_preview::AnimalismPower,
                        purchases::{PurchaseRequest, TeacherRule},
                    };
                    self.purchase_form.input = PurchaseRequest {
                        review_id: None,
                        id: uuid::Uuid::new_v4().to_string(),
                        power_id: AnimalismPower::ALL[2].stable_id().into(),
                        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
                        relation: Some(ClanRelation::OutOfClan),
                        affiliation_name: "Synthetic clan".into(),
                        day: "2026-01-02".into(),
                        available_from: "2026-01-03".into(),
                        next_session_reference: "Synthetic next session".into(),
                        actor: "Synthetic ST".into(),
                        reason: "Synthetic source-bound purchase".into(),
                        policy_version: "Synthetic1".into(),
                        policy_reference: "Synthetic policy".into(),
                        affiliation_evidence: "Synthetic affiliation review".into(),
                        approval_reference: "Synthetic local approval".into(),
                        eligibility_evidence: "Synthetic eligibility review".into(),
                        teacher_rule: Some(TeacherRule::Required),
                        teacher_evidence: "Synthetic named teacher one level above Basic".into(),
                        standard_cost_confirmed: true,
                        owbn_evidence: Default::default(),
                    };
                    if !smoke.checks.contains_key("policy_template_file_reuse") {
                        let path = smoke.output_dir.join("Synthetic local policy.gvpolicy");
                        self.purchase_form.policy_name = "Synthetic reusable policy".into();
                        let original = self.purchase_form.input.clone();
                        let revision = self.library.as_ref().unwrap().revision().unwrap();
                        let outcome = (|| -> grapevine_workbench::model::Result<()> {
                            let template = grapevine_workbench::policy_templates::PolicyTemplate::from_request(
                                &self.purchase_form.policy_name,
                                &original,
                            )?;
                            std::fs::write(&path, template.to_bytes()?)?;
                            let loaded =
                                grapevine_workbench::policy_templates::PolicyTemplate::parse(
                                    &std::fs::read(&path)?,
                                )?;
                            loaded.apply(&mut self.purchase_form.input)?;
                            self.purchase_form.quote = None;
                            Ok(())
                        })();
                        let fresh = self.purchase_form.input.approval_reference.is_empty()
                            && self.purchase_form.input.eligibility_evidence.is_empty()
                            && self.purchase_form.input.teacher_evidence.is_empty()
                            && self.purchase_form.quote.is_none();
                        let valid = outcome.is_ok()
                            && fresh
                            && self.purchase_form.input.id == original.id
                            && self.purchase_form.input.policy_version == original.policy_version
                            && self.library.as_ref().unwrap().revision().unwrap() == revision;
                        smoke.checks.insert(
                            "policy_template_file_reuse".into(),
                            serde_json::json!(valid),
                        );
                        if !valid {
                            smoke.failure =
                                Some(format!("Policy template reuse failed: {outcome:?}"));
                            smoke.phase = SmokePhase::Capture;
                            return;
                        }
                        // Each purchase needs fresh, deliberately supplied synthetic review.
                        self.purchase_form.input.approval_reference = original.approval_reference;
                        self.purchase_form.input.eligibility_evidence =
                            original.eligibility_evidence;
                        self.purchase_form.input.teacher_evidence = original.teacher_evidence;
                    }
                    self.purchase_form.dirty = true;
                    if let Some(rect) = probe(ctx, "review_save") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::ReviewSaved;
                    }
                }
            }
            SmokePhase::ReviewSaved => {
                let saved = self.purchase_form.reviews.len() == 1
                    && self.purchase_form.reviews[0].1
                    && self.purchase_form.input.review_id.is_some()
                    && self.purchase_form.input.approval_reference.is_empty()
                    && self.purchase_form.input.teacher_evidence.is_empty();
                smoke.checks.insert(
                    "review_native_saved_private".into(),
                    serde_json::json!(saved),
                );
                if !saved {
                    smoke.failure = Some(format!(
                        "Native review save failed: {:?}",
                        self.purchase_form.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                } else if let Some(rect) = probe(ctx, "review_reuse_latest") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ReviewReused;
                }
            }
            SmokePhase::ReviewReused => {
                let current = self.purchase_form.input.review_id.as_ref()
                    == self.purchase_form.reviews.last().map(|(r, _)| &r.id)
                    && self.purchase_form.input.approval_reference.is_empty()
                    && self.purchase_form.input.teacher_evidence.is_empty()
                    && !self.purchase_form.input.eligibility_evidence.is_empty()
                    && self.purchase_form.quote.is_none();
                smoke.checks.insert(
                    "review_native_reuse_fresh_approval".into(),
                    serde_json::json!(current),
                );
                if !current {
                    smoke.failure = Some(
                        "Reusing review failed to retain scoped evidence and clear approval".into(),
                    );
                    smoke.phase = SmokePhase::Capture;
                } else {
                    self.purchase_form.input.approval_reference =
                        "Fresh synthetic approval after review".into();
                    self.purchase_form.input.teacher_evidence =
                        "Fresh synthetic qualified teacher after review".into();
                    if let Some(rect) = probe(ctx, "purchase_quote") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::PurchaseQuoted;
                    }
                }
            }
            SmokePhase::PurchaseQuoted => {
                if let Some(q) = self.purchase_form.quote.as_ref() {
                    let bound = q.before_unspent == "10.25"
                        && q.after_unspent == "6.25"
                        && q.price.price.base == "3"
                        && q.price.price.affiliation_adjustment == "1"
                        && q.price.price.total == "4";
                    let new_family = q.price.power.family() == "Auspex"
                        && q.price.power_source.citation.printed_page == 137;
                    smoke.checks.insert(
                        "purchase_native_new_family".into(),
                        serde_json::json!(new_family),
                    );
                    smoke
                        .checks
                        .insert("purchase_quote_bindings".into(), serde_json::json!(bound));
                    if !bound {
                        smoke.failure =
                            Some("Purchase quote had wrong exact cost or balance".into());
                        smoke.phase = SmokePhase::Capture;
                    } else if let Some(rect) = probe(ctx, "purchase_commit") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::PurchaseCommitted;
                    }
                } else {
                    smoke.failure = Some(format!(
                        "Synthetic purchase quote failed: {:?}",
                        self.purchase_form.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::PurchaseCommitted => {
                let saved = self.purchase_form.receipts.len() == 1
                    && self.purchase_form.receipts[0]
                        .quote
                        .request
                        .review_id
                        .is_some()
                    && self.purchase_form.reviews.len() == 1
                    && !self.purchase_form.reviews[0].1
                    && !self.purchase_form.dirty
                    && self.purchase_form.quote.is_none();
                smoke
                    .checks
                    .insert("purchase_native_commit".into(), serde_json::json!(saved));
                if !saved {
                    smoke.failure = Some("Native purchase did not save linked receipt".into());
                    smoke.phase = SmokePhase::Capture;
                } else {
                    let path = smoke.output_dir.join("purchase-receipt-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::PurchaseDone;
                }
            }
            SmokePhase::PurchaseDone => {
                if smoke
                    .output_dir
                    .join("purchase-receipt-smoke.bmp")
                    .is_file()
                {
                    self.purchase_open = false;
                    smoke.phase = SmokePhase::PrintClick;
                }
            }
            SmokePhase::PrintClick => {
                if let Some(rect) = self.smoke_controls.print {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PrintOpen;
                }
            }
            SmokePhase::PrintOpen => {
                if self.print_open {
                    self.print_form.options.large_text = true;
                    self.print_form.options.language =
                        grapevine_workbench::printing::DocLanguage::English;
                    if let Some(rect) = probe(ctx, "print_mechanics_toggle") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::PrintMechanicsEnabled;
                    }
                }
            }
            SmokePhase::PrintMechanicsEnabled => {
                if self.print_form.options.include_simple_mechanics {
                    smoke.checks.insert(
                        "print_native_mechanics_option_event".into(),
                        serde_json::json!(true),
                    );
                    if let Some(rect) = probe(ctx, "print_preview") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::PrintReady;
                    }
                }
            }
            SmokePhase::PrintReady => {
                if let Some(doc) = self.print_form.document.as_ref() {
                    let lang = self.language == Language::Portuguese
                        && doc.options.language
                            == grapevine_workbench::printing::DocLanguage::English
                        && doc.options.large_text;
                    let source = doc.references.iter().any(|r| {
                        r.power == "Heightened Senses"
                            && r.family == "Auspex"
                            && r.printed_page == 137
                            && r.pdf_page == 138
                    });
                    let html = grapevine_workbench::printing::html(doc).unwrap();
                    let safe = !html.contains("SMOKE_ONLY_SECRET")
                        && !html.contains("SMOKE_TRAIT_SECRET")
                        && !html.contains("Synthetic approval")
                        && doc.scope == grapevine_workbench::printing::PrintScope::Public;
                    let unchanged = self.library.as_ref().and_then(|l| l.revision().ok())
                        == self.print_form.revision;
                    let path = smoke.output_dir.join("character-sheet.html");
                    let exported =
                        grapevine_workbench::safe_files::write_new(&path, html.as_bytes()).is_ok()
                            && std::fs::read_to_string(&path).ok().as_ref() == Some(&html);
                    smoke.checks.insert(
                        "print_independent_language_large_text".into(),
                        serde_json::json!(lang),
                    );
                    smoke.checks.insert(
                        "print_verified_owned_source".into(),
                        serde_json::json!(source),
                    );
                    smoke.checks.insert(
                        "print_public_privacy_readonly_export".into(),
                        serde_json::json!(safe && unchanged && exported),
                    );
                    if !(lang && source && safe && unchanged && exported) {
                        smoke.failure =
                            Some("Printable projection/source/privacy/export check failed".into());
                        smoke.phase = SmokePhase::Capture;
                    } else {
                        let path = smoke.output_dir.join("print-preview-smoke.bmp");
                        ctx.request_screenshot(move |image| {
                            let _ = write_bmp(&path, &image);
                        });
                        smoke.phase = SmokePhase::PrintCloseAfterCapture;
                    }
                } else {
                    smoke.failure = Some(format!(
                        "Print preview failed: {:?}",
                        self.print_form.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::PrintCloseAfterCapture => {
                if smoke.output_dir.join("print-preview-smoke.bmp").is_file() {
                    self.print_open = false;
                    self.purchase_open = true;
                    smoke.phase = SmokePhase::CancelSelect;
                }
            }
            SmokePhase::CancelSelect => {
                if let Some(rect) = probe(ctx, "cancel_select") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::CancelOpen;
                }
            }
            SmokePhase::CancelOpen => {
                if self.purchase_form.cancel_purchase.is_some() {
                    self.purchase_form.cancel_input.day = "2026-01-03".into();
                    self.purchase_form.cancel_input.actor = "Synthetic ST".into();
                    self.purchase_form.cancel_input.reason = "Synthetic correction".into();
                    self.purchase_form.cancel_input.evidence =
                        "Synthetic cancellation approval".into();
                    if let Some(rect) = probe(ctx, "cancel_quote") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::CancelQuoted;
                    }
                }
            }
            SmokePhase::CancelQuoted => {
                if let Some(q) = self.purchase_form.cancel_quote.as_ref() {
                    let ok =
                        q.refund == "4" && q.before_unspent == "6.25" && q.after_unspent == "10.25";
                    smoke.checks.insert(
                        "cancellation_original_cost_quote".into(),
                        serde_json::json!(ok),
                    );
                    if !ok {
                        smoke.failure =
                            Some("Cancellation quote did not refund original4XP".into());
                        smoke.phase = SmokePhase::Capture;
                    } else if let Some(rect) = probe(ctx, "cancel_commit") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::CancelCommitted;
                    }
                } else {
                    smoke.failure = Some(format!(
                        "Synthetic cancellation quote failed: {:?}",
                        self.purchase_form.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::CancelCommitted => {
                let ok = self.purchase_form.receipts.len() == 1
                    && self.purchase_form.receipts[0].cancellation.is_some()
                    && !self.purchase_form.dirty;
                smoke.checks.insert(
                    "cancellation_native_refund_and_removal".into(),
                    serde_json::json!(ok),
                );
                if !ok {
                    smoke.failure =
                        Some("Cancellation did not save original receipt with refund".into());
                    smoke.phase = SmokePhase::Capture;
                } else {
                    let path = smoke.output_dir.join("cancellation-receipt-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::CancelDone;
                }
            }
            SmokePhase::CancelDone => {
                smoke.batch_previous_selection = self.selected_id.clone();
                let mut library = Library::create(
                    &smoke.output_dir.join("batch-only.gvlib"),
                    "Synthetic advancement",
                )
                .unwrap();
                let id = library
                    .create_character("Vampire", "Synthetic advancement", 0, Role::Storyteller)
                    .unwrap();
                library
                    .initialize_xp(
                        &id,
                        &grapevine_workbench::xp::Baseline {
                            day: "2026-01-01".into(),
                            earned: "20.25".into(),
                            unspent: "20.25".into(),
                            actor: "Synthetic ST".into(),
                            reason: "Synthetic opening".into(),
                            evidence: "Synthetic fixture".into(),
                        },
                        1,
                        Role::Storyteller,
                    )
                    .unwrap();
                smoke.phase = SmokePhase::BatchAddFirst;
                self.set_library(library);
                self.language = Language::Portuguese;
                self.open_purchases();
                self.purchase_form.input = smoke_batch_request(false);
                self.purchase_form.dirty = true;
            }
            SmokePhase::BatchAddFirst | SmokePhase::BatchAddSecond => {
                if let Some(rect) = probe(ctx, "purchase_plan_add") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = if smoke.phase == SmokePhase::BatchAddFirst {
                        SmokePhase::BatchFirstAdded
                    } else {
                        SmokePhase::BatchSecondAdded
                    };
                } else {
                    scroll_xp(&mut self.pending_raw_events, ctx, -350.0);
                }
            }
            SmokePhase::BatchFirstAdded => {
                if self.purchase_form.plan.len() == 1 {
                    self.purchase_form.input = smoke_batch_request(true);
                    self.purchase_form.dirty = true;
                    smoke.phase = SmokePhase::BatchAddSecond;
                }
            }
            SmokePhase::BatchSecondAdded => {
                let valid = self.purchase_form.plan_quote.as_ref().is_some_and(|q| {
                    q.steps.len() == 2 && q.total_spend == "7" && q.after_unspent == "13.25"
                }) && self.library.as_ref().unwrap().revision().unwrap() == 2
                    && self
                        .library
                        .as_ref()
                        .unwrap()
                        .purchase_receipts(self.selected_id.as_ref().unwrap(), Role::Storyteller)
                        .unwrap()
                        .is_empty();
                smoke.checks.insert(
                    "batch_native_cumulative_readonly_preview".into(),
                    serde_json::json!(valid),
                );
                if valid {
                    smoke.phase = SmokePhase::BatchApply;
                } else {
                    smoke.failure = Some(format!(
                        "Advancement plan preview failed: {:?}",
                        self.purchase_form.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::BatchApply => {
                if let Some(rect) = probe(ctx, "purchase_plan_commit") {
                    let path = smoke.output_dir.join("advancement-plan-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::BatchApplied;
                } else {
                    scroll_xp(&mut self.pending_raw_events, ctx, 350.0);
                }
            }
            SmokePhase::BatchApplied => {
                let library = self.library.as_ref().unwrap();
                let valid =
                    self.purchase_form.plan.is_empty()
                        && self.purchase_form.receipts.len() == 2
                        && self.purchase_form.receipts.iter().all(|receipt| {
                            receipt.batch.is_some() && receipt.committed_revision == 3
                        })
                        && library.revision().unwrap() == 3
                        && library
                            .xp_view(self.selected_id.as_ref().unwrap(), Role::Storyteller)
                            .unwrap()
                            .unspent
                            .as_deref()
                            == Some("13.25");
                smoke.checks.insert(
                    "batch_native_atomic_apply_and_receipts".into(),
                    serde_json::json!(valid),
                );
                if valid {
                    smoke.phase = SmokePhase::AdvancementQueue;
                    self.purchase_form.input = smoke_batch_request(false);
                    self.purchase_form.input.power_id =
                        "met.lotn-revised.auspex.advanced.psychic_projection".into();
                    self.purchase_form.input.relation =
                        Some(grapevine_workbench::pricing::ClanRelation::OutOfClan);
                    self.purchase_form.dirty = true;
                } else {
                    smoke.failure = Some(format!(
                        "Advancement commit failed: {:?}",
                        self.purchase_form.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::AdvancementQueue => {
                if let Some(rect) = probe(ctx, "purchase_plan_add") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::AdvancementSave;
                } else {
                    scroll_xp(&mut self.pending_raw_events, ctx, -350.0);
                }
            }
            SmokePhase::AdvancementSave => {
                if self.purchase_form.plan.len() == 1 {
                    if let Some(rect) = probe(ctx, "purchase_plan_save") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::AdvancementSaved;
                    } else {
                        scroll_xp(&mut self.pending_raw_events, ctx, 350.0);
                    }
                }
            }
            SmokePhase::AdvancementSaved => {
                if self.advancement.open
                    && self.route == WorkArea::Builder
                    && let Some(rect) = probe(ctx, "advancement_draft_prepare")
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::AdvancementReview;
                }
            }
            SmokePhase::AdvancementReview => {
                let valid = self.advancement.review.as_ref().is_some_and(|review| {
                    !review.applied
                        && review.quote.steps.len() == 1
                        && review.quote.before_unspent == "13.25"
                        && review.quote.after_unspent == "3.25"
                        && review.quote.steps[0].price.tier
                            == grapevine_workbench::pricing::MetTier::Advanced
                        && review.quote.steps[0]
                            .price
                            .power_source
                            .citation
                            .printed_page
                            == 139
                }) && self.library.as_ref().unwrap().revision().unwrap() == 4;
                smoke.checks.insert(
                    "advancement_native_saved_draft_review".into(),
                    serde_json::json!(valid),
                );
                if !valid {
                    smoke.failure = Some(format!(
                        "Saved advancement review failed: {:?}",
                        self.advancement.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                } else if let Some(rect) = probe(ctx, "advancement_draft_apply") {
                    let path = smoke.output_dir.join("saved-advancement-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::AdvancementApplied;
                } else {
                    scroll_xp(&mut self.pending_raw_events, ctx, -250.0);
                }
            }
            SmokePhase::AdvancementApplied => {
                let lib = self.library.as_ref().unwrap();
                let valid = self.advancement.review.as_ref().is_some_and(|review| {
                    review.applied
                        && lib
                            .xp_view(&review.quote.record_id, Role::Storyteller)
                            .unwrap()
                            .unspent
                            .as_deref()
                            == Some("3.25")
                        && lib
                            .purchase_receipts(&review.quote.record_id, Role::Storyteller)
                            .unwrap()
                            .len()
                            == 3
                }) && lib.revision().unwrap() == 5;
                smoke.checks.insert(
                    "advancement_native_live_apply_and_marker".into(),
                    serde_json::json!(valid),
                );
                if valid {
                    let id = self
                        .advancement
                        .review
                        .as_ref()
                        .unwrap()
                        .quote
                        .record_id
                        .clone();
                    smoke.phase = SmokePhase::PublishedClick;
                    self.change_route(WorkArea::Storyteller);
                    self.select_record(id);
                } else {
                    smoke.failure = Some(format!(
                        "Saved advancement apply failed: {:?}",
                        self.advancement.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::PublishedClick => {
                if let Some(rect) = probe(ctx, "published_open") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PublishedOpen;
                }
            }
            SmokePhase::PublishedOpen => {
                if self.published.open && self.published.context.is_some() {
                    self.published.request.trait_index = 2;
                    self.published.request.source_id =
                        "v20-core:discipline:auspex:5:psychic-projection".into();
                    self.published.request.day = "2026-01-03".into();
                    self.published.request.actor = "PRIVATE_NATIVE_SOURCE_ACTOR".into();
                    self.published.request.reason = "PRIVATE_NATIVE_SOURCE_REASON".into();
                    self.published.book_id = Some("v20-core".into());
                    self.published.query = "Psychic Projection".into();
                    self.published.dirty = true;
                    smoke.phase = SmokePhase::PublishedPrepare;
                }
            }
            SmokePhase::PublishedPrepare => {
                if let Some(rect) = probe(ctx, "published_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PublishedQuoted;
                } else {
                    scroll_xp(&mut self.pending_raw_events, ctx, -300.0);
                }
            }
            SmokePhase::PublishedQuoted => {
                let valid = self.published.quote.as_ref().is_some_and(|quote| {
                    quote.reference.as_ref().is_some_and(|reference| {
                        reference.power.level == 5
                            && reference.edition
                                == grapevine_workbench::published_powers::Edition::V20
                    }) && quote.old_value == "advanced"
                        && quote.updated_trait["Value"] == "advanced"
                });
                if !valid {
                    smoke.failure = Some(format!(
                        "Published reference quote failed: {:?}",
                        self.published.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                } else if let Some(rect) = probe(ctx, "published_commit") {
                    let path = smoke.output_dir.join("published-reference-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PublishedSaved;
                } else {
                    scroll_xp(&mut self.pending_raw_events, ctx, -300.0);
                }
            }
            SmokePhase::PublishedSaved => {
                let lib = self.library.as_ref().unwrap();
                let id = self.selected_id.as_ref().unwrap();
                let valid = self.published.context.as_ref().is_some_and(|context| {
                    context.history.len() == 1
                        && context.traits[2]["Value"] == "advanced"
                        && grapevine_workbench::published_reference::verified_reference(
                            &context.traits[2],
                        )
                        .is_some_and(|reference| reference.power.level == 5)
                }) && lib
                    .xp_view(id, Role::Storyteller)
                    .unwrap()
                    .unspent
                    .as_deref()
                    == Some("3.25")
                    && lib.purchase_receipts(id, Role::Storyteller).unwrap().len() == 3;
                let doc = lib
                    .print_document(
                        id,
                        &grapevine_workbench::printing::Options {
                            scope: grapevine_workbench::printing::PrintScope::PrivateStoryteller,
                            ..Default::default()
                        },
                        lib.revision().unwrap(),
                        Role::Storyteller,
                    )
                    .unwrap();
                let json = serde_json::to_string(&doc).unwrap();
                let public = serde_json::to_string(&lib.record(id, Role::Player).unwrap()).unwrap();
                let privacy = doc.published_references.len() == 1
                    && doc.published_references[0].source.power.level == 5
                    && !json.contains("PRIVATE_NATIVE_SOURCE_")
                    && !public.contains("PRIVATE_NATIVE_SOURCE_");
                smoke.checks.insert(
                    "published_source_native_link_preserves_xp_and_tier".into(),
                    serde_json::json!(valid),
                );
                smoke.checks.insert(
                    "published_source_native_reference_privacy".into(),
                    serde_json::json!(privacy),
                );
                if valid && privacy {
                    self.published = Default::default();
                    smoke.phase = SmokePhase::OrdinaryClick;
                } else {
                    smoke.failure = Some(format!(
                        "Published source save/privacy failed: {:?}",
                        self.published.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::OrdinaryClick => {
                if let Some(rect) = probe(ctx, "ordinary_open") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::OrdinaryOpen;
                }
            }
            SmokePhase::OrdinaryOpen => {
                if self.ordinary.open && self.ordinary.context.is_some() {
                    self.ordinary.request.category =
                        grapevine_workbench::trait_pricing::Category::Willpower;
                    self.ordinary.field = "Willpower".into();
                    self.ordinary.request.profile = Some(
                        grapevine_workbench::pricing::PricingProfile::MetLawsOfTheNightRevised,
                    );
                    self.ordinary.request.day = "2026-01-03".into();
                    self.ordinary.request.actor = "PRIVATE_ORDINARY_ACTOR".into();
                    self.ordinary.request.reason = "PRIVATE_ORDINARY_REASON".into();
                    self.ordinary.request.policy_version = "synthetic-policy-v1".into();
                    self.ordinary.request.review_reference =
                        "Synthetic eligibility, maximum and local ST approval".into();
                    self.ordinary.request.review_confirmed = true;
                    self.ordinary.request.show_on_player_sheet = true;
                    smoke.phase = SmokePhase::OrdinaryPrepare;
                }
            }
            SmokePhase::OrdinaryPrepare => {
                if let Some(rect) = probe(ctx, "ordinary_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::OrdinaryQuoted;
                } else if let Some(rect) = probe(ctx, "ordinary_scroll") {
                    self.pending_raw_events
                        .push(egui::Event::PointerMoved(rect.center()));
                    self.pending_raw_events.push(egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Unknown,
                        delta: egui::vec2(0.0, -250.0),
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
            SmokePhase::OrdinaryQuoted => {
                if self.ordinary.quote.as_ref().is_some_and(|q| {
                    q.price.cost == "3"
                        && q.price.current == 0
                        && q.price.next == 1
                        && q.after_unspent == "0.25"
                }) {
                    if let Some(rect) = probe(ctx, "ordinary_commit") {
                        let path = smoke.output_dir.join("ordinary-trait-smoke.bmp");
                        ctx.request_screenshot(move |image| {
                            let _ = write_bmp(&path, &image);
                        });
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::OrdinarySaved;
                    } else if let Some(rect) = probe(ctx, "ordinary_scroll") {
                        self.pending_raw_events
                            .push(egui::Event::PointerMoved(rect.center()));
                        self.pending_raw_events.push(egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            phase: egui::TouchPhase::Move,
                            source: egui::MouseWheelSource::Unknown,
                            delta: egui::vec2(0.0, -250.0),
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                } else {
                    smoke.failure = Some(format!(
                        "Ordinary advancement quote failed: {:?}",
                        self.ordinary.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::OrdinarySaved => {
                let lib = self.library.as_ref().unwrap();
                let id = self.selected_id.as_ref().unwrap();
                let valid = self.ordinary.context.as_ref().is_some_and(|c| {
                    c.history.len() == 1 && c.sheet.resources.get("Willpower") == Some(&1)
                }) && lib.xp_view(id, Role::Storyteller).is_ok_and(|view| {
                    view.unspent.as_deref() == Some("0.25")
                        && !view
                            .findings
                            .iter()
                            .any(|f| f.code == "TRAIT_ADVANCEMENT_SHEET_DRIFT")
                });
                let public = lib
                    .print_document(
                        id,
                        &grapevine_workbench::printing::Options::default(),
                        self.revision.unwrap(),
                        Role::Player,
                    )
                    .ok()
                    .and_then(|doc| serde_json::to_string(&doc).ok());
                let privacy = public.is_some_and(|text| {
                    !text.contains("PRIVATE_ORDINARY_ACTOR")
                        && !text.contains("PRIVATE_ORDINARY_REASON")
                }) && lib.record(id, Role::Player).is_ok_and(|view| {
                    view.fields
                        .get("Willpower")
                        .and_then(serde_json::Value::as_i64)
                        == Some(1)
                });
                smoke.checks.insert(
                    "ordinary_trait_native_atomic_xp_and_count".into(),
                    serde_json::json!(valid),
                );
                smoke.checks.insert(
                    "ordinary_trait_native_private_receipt_public_resource".into(),
                    serde_json::json!(privacy),
                );
                smoke.phase = if valid && privacy {
                    SmokePhase::CorrectionSelect
                } else {
                    smoke.failure = Some("Ordinary advancement save/privacy failed".into());
                    SmokePhase::Capture
                };
            }
            SmokePhase::CorrectionSelect => {
                if let Some(rect) = probe(ctx, "ordinary_correct") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::CorrectionOpen;
                } else if let Some(rect) = probe(ctx, "ordinary_scroll") {
                    scroll_at(&mut self.pending_raw_events, rect.center(), -250.0);
                }
            }
            SmokePhase::CorrectionOpen => {
                if let Some(request) = &mut self.ordinary.correction {
                    request.day = "2026-01-04".into();
                    request.actor = "PRIVATE_CORRECTION_ACTOR".into();
                    request.reason = "Synthetic mistaken increment".into();
                    request.evidence = "PRIVATE_CORRECTION_EVIDENCE".into();
                    smoke.phase = SmokePhase::CorrectionPrepare;
                }
            }
            SmokePhase::CorrectionPrepare => {
                if let Some(rect) = probe(ctx, "ordinary_correction_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::CorrectionQuoted;
                } else if let Some(rect) = probe(ctx, "ordinary_scroll") {
                    scroll_at(&mut self.pending_raw_events, rect.center(), -250.0);
                }
            }
            SmokePhase::CorrectionQuoted => {
                if self.ordinary.correction_quote.as_ref().is_some_and(|q| {
                    q.refund == "3" && q.before_unspent == "0.25" && q.after_unspent == "3.25"
                }) {
                    if let Some(rect) = probe(ctx, "ordinary_correction_commit") {
                        let path = smoke.output_dir.join("ordinary-correction-smoke.bmp");
                        ctx.request_screenshot(move |image| {
                            let _ = write_bmp(&path, &image);
                        });
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::CorrectionSaved;
                    } else if let Some(rect) = probe(ctx, "ordinary_scroll") {
                        scroll_at(&mut self.pending_raw_events, rect.center(), -250.0);
                    }
                } else {
                    smoke.failure = Some(format!(
                        "Paired correction preview failed: {:?}",
                        self.ordinary.message
                    ));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::CorrectionSaved => {
                let lib = self.library.as_ref().unwrap();
                let id = self.selected_id.as_ref().unwrap();
                let valid = self.ordinary.context.as_ref().is_some_and(|c| {
                    c.corrected_ids.len() == 1 && c.sheet.resources.get("Willpower") == Some(&0)
                }) && lib.xp_view(id, Role::Storyteller).is_ok_and(|v| {
                    v.unspent.as_deref() == Some("3.25")
                        && !v
                            .findings
                            .iter()
                            .any(|f| f.code == "TRAIT_ADVANCEMENT_SHEET_DRIFT")
                });
                let privacy = lib
                    .print_document(
                        id,
                        &grapevine_workbench::printing::Options::default(),
                        self.revision.unwrap(),
                        Role::Player,
                    )
                    .ok()
                    .and_then(|doc| serde_json::to_string(&doc).ok())
                    .is_some_and(|s| {
                        !s.contains("PRIVATE_CORRECTION_ACTOR")
                            && !s.contains("PRIVATE_CORRECTION_EVIDENCE")
                    });
                smoke.checks.insert(
                    "ordinary_correction_native_paired_refund_and_count".into(),
                    serde_json::json!(valid),
                );
                smoke.checks.insert(
                    "ordinary_correction_native_private_evidence".into(),
                    serde_json::json!(privacy),
                );
                smoke.phase = if valid && privacy {
                    SmokePhase::CreationOpen
                } else {
                    smoke.failure = Some("Paired correction save/privacy failed".into());
                    SmokePhase::Capture
                };
            }
            SmokePhase::CreationOpen => {
                let mut library = Library::create(
                    &smoke.output_dir.join("creation-only.gvlib"),
                    "Synthetic creation",
                )
                .unwrap();
                let id = library
                    .create_character("Vampire", "Synthetic new Vampire", 0, Role::Storyteller)
                    .unwrap();
                smoke.phase = SmokePhase::CreationSourceChoose;
                self.set_library(library);
                self.selected_id = Some(id.clone());
                self.load_record(&id, false);
                self.language = Language::English;
                self.open_vampire_creation();
                if let Some(r) = self.creation.request.as_mut() {
                    for (key, rows) in [
                        (
                            "PhysicalList",
                            serde_json::json!([{"Name":"Strong","Value":"7","Note":"PRIVATE_CREATION_TRAIT"}]),
                        ),
                        (
                            "SocialList",
                            serde_json::json!([{"Name":"Charming","Value":"5","Note":""}]),
                        ),
                        (
                            "MentalList",
                            serde_json::json!([{"Name":"Alert <literal>","Value":"3","Note":""}]),
                        ),
                        (
                            "AbilityList",
                            serde_json::json!([{"Name":"Academics","Value":"5","Note":""}]),
                        ),
                        (
                            "BackgroundList",
                            serde_json::json!([{"Name":"Generation","Value":"2","Note":""},{"Name":"Resources","Value":"3","Note":""}]),
                        ),
                        (
                            "DisciplineList",
                            serde_json::json!([{"Name":"Synthetic first","Value":"basic","Note":""},{"Name":"Synthetic second","Value":"basic","Note":""},{"Name":"Synthetic third","Value":"basic","Note":""}]),
                        ),
                    ] {
                        r.sheet.lists.get_mut(key).unwrap()["Traits"] = rows;
                    }
                    r.budget.virtue_allocated = [3, 2, 2];
                    r.standard_pools = true;
                    r.review_confirmed = true;
                    r.actor = "PRIVATE_CREATION_ACTOR".into();
                    r.day = "2026-11-13".into();
                    r.reason = "PRIVATE_CREATION_REASON".into();
                    r.policy_version = "Synthetic v1".into();
                    r.review_reference =
                        "Synthetic clan, merits, progression and eligibility review".into();
                    r.manual_power_reference =
                        "Synthetic edition/page and MET Basic mapping".into();
                    self.creation.dirty = true;
                    self.creation.picker.query = "Feral Whispers".into();
                } else {
                    if let Some(smoke) = self.smoke.as_mut() {
                        smoke.failure =
                            Some("Creation screen did not load a native Vampire".into());
                        smoke.phase = SmokePhase::Capture;
                    }
                }
            }
            SmokePhase::CreationSourceChoose => {
                if let Some(rect) = probe(ctx, "creation_source_result") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::CreationSourceChosen;
                }
            }
            SmokePhase::CreationSourceChosen => {
                if let Some(request) = self.creation.request.as_mut() {
                    let row =
                        &mut request.sheet.lists.get_mut("DisciplineList").unwrap()["Traits"][0];
                    if row["Name"] == "Feral Whispers" {
                        let key = grapevine_workbench::creation_sources::PROPOSED;
                        grapevine_workbench::creation_sources::choose_met(
                            row,
                            Some(
                                grapevine_workbench::purchase_preview::ANIMALISM_FERAL_WHISPERS_ID,
                            ),
                        )
                        .unwrap();
                        row[key]["reviewed_signature"] = serde_json::json!(
                            grapevine_workbench::creation_sources::review_signature(row).unwrap()
                        );
                        smoke.phase = SmokePhase::CreationPrepare;
                    }
                }
            }
            SmokePhase::CreationPrepare => {
                if let Some(rect) = probe(ctx, "creation_preview") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::CreationQuoted;
                }
            }
            SmokePhase::CreationQuoted => {
                if let Some(q) = self.creation.quote.as_ref() {
                    if !q.ready {
                        smoke.failure =
                            Some(format!("Creation preview was incomplete: {:?}", q.issues));
                        smoke.phase = SmokePhase::Capture;
                    } else if let Some(rect) = probe(ctx, "creation_apply") {
                        let path = smoke.output_dir.join("creation-preview-smoke.bmp");
                        ctx.request_screenshot(move |image| {
                            let _ = write_bmp(&path, &image);
                        });
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::CreationSaved;
                    }
                }
            }
            SmokePhase::CreationSaved => {
                let lib = self.library.as_ref().unwrap();
                let id = self.selected_id.as_ref().unwrap();
                if lib
                    .vampire_creation_context(id, Role::Storyteller)
                    .is_ok_and(|c| c.history.len() == 1)
                {
                    let ctx = lib.vampire_creation_context(id, Role::Storyteller).unwrap();
                    let values = ctx.history[0].quote.budget.virtues == [4, 3, 3]
                        && ctx.history[0].quote.budget.free_remaining == 5
                        && ctx.history[0]
                            .quote
                            .effective_sheet
                            .resources
                            .get("Willpower")
                            == Some(&4)
                        && lib
                            .xp_view(id, Role::Storyteller)
                            .is_ok_and(|v| v.ledger.is_none());
                    let privacy = lib
                        .print_document(
                            id,
                            &grapevine_workbench::printing::Options::default(),
                            lib.revision().unwrap(),
                            Role::Player,
                        )
                        .ok()
                        .and_then(|d| serde_json::to_string(&d).ok())
                        .is_some_and(|s| s.contains("Strong") && !s.contains("PRIVATE_CREATION"));
                    let reopened = Library::open(&lib.path)
                        .and_then(|l| l.vampire_creation_context(id, Role::Storyteller))
                        .is_ok_and(|c| {
                            c.history.len() == 1
                                && c.history[0].quote.effective_sheet.resources.get("Blood")
                                    == Some(&12)
                        });
                    for (label, language) in [
                        ("en", grapevine_workbench::printing::DocLanguage::English),
                        ("pt", grapevine_workbench::printing::DocLanguage::Portuguese),
                    ] {
                        let options = grapevine_workbench::printing::Options {
                            language,
                            ..Default::default()
                        };
                        let document = lib
                            .print_document(id, &options, lib.revision().unwrap(), Role::Player)
                            .unwrap();
                        let html = grapevine_workbench::printing::html(&document).unwrap();
                        grapevine_workbench::safe_files::write_new(
                            &smoke.output_dir.join(format!("creation-{label}.html")),
                            html.as_bytes(),
                        )
                        .unwrap();
                    }
                    smoke.checks.insert(
                        "creation_native_profile_preview_and_apply".into(),
                        serde_json::json!(values),
                    );
                    smoke.checks.insert(
                        "creation_native_public_print_privacy".into(),
                        serde_json::json!(privacy),
                    );
                    smoke.checks.insert(
                        "creation_native_reopen_no_xp_grant".into(),
                        serde_json::json!(reopened),
                    );
                    let source_saved = ctx.history[0].quote.published_sources.len() == 1
                        && lib
                            .published_reference_context(id, Role::Storyteller)
                            .is_ok_and(|c| c.history.len() == 1)
                        && lib
                            .print_document(
                                id,
                                &Default::default(),
                                lib.revision().unwrap(),
                                Role::Player,
                            )
                            .is_ok_and(|d| {
                                d.published_references.len() == 1
                                    && d.published_references[0].source.power.name
                                        == "Feral Whispers"
                            });
                    smoke.checks.insert(
                        "creation_native_source_selection_and_review".into(),
                        serde_json::json!(source_saved),
                    );
                    let mechanics = lib
                        .print_document(
                            id,
                            &grapevine_workbench::printing::Options {
                                include_simple_mechanics: true,
                                ..Default::default()
                            },
                            lib.revision().unwrap(),
                            Role::Player,
                        )
                        .is_ok_and(|d| {
                            d.sections
                                .iter()
                                .filter(|s| s.title.starts_with("Simple mechanics:"))
                                .count()
                                == 1
                                && d.sections
                                    .iter()
                                    .any(|s| s.title.ends_with("Feral Whispers"))
                                && !serde_json::to_string(&d)
                                    .unwrap()
                                    .contains("PRIVATE_CREATION")
                        });
                    smoke.checks.insert(
                        "creation_native_free_met_mapping_mechanics".into(),
                        serde_json::json!(
                            mechanics && ctx.history[0].quote.met_mappings.len() == 1
                        ),
                    );
                    smoke.phase = if values && privacy && reopened && source_saved && mechanics {
                        SmokePhase::MetOpen
                    } else {
                        smoke.failure =
                            Some("Creation native saved values/privacy/reopen failed".into());
                        SmokePhase::Capture
                    };
                }
            }
            SmokePhase::MetOpen => {
                smoke.phase = SmokePhase::MetPrepare;
                self.creation.open = false;
                self.open_met_reference();
                self.met_reference.request = grapevine_workbench::met_references::Request {
                    trait_index: 1,
                    power_id: grapevine_workbench::purchase_preview::ANIMALISM_BECKONING_ID.into(),
                    day: "2026-11-14".into(),
                    actor: "PRIVATE_MET_NATIVE_ACTOR".into(),
                    reason: "PRIVATE_MET_NATIVE_REASON".into(),
                    review_reference: "PRIVATE_MET_NATIVE_REVIEW".into(),
                    confirmed: true,
                    confirm_alias: true,
                    confirm_legacy_value: true,
                    ..Default::default()
                };
                self.met_reference.dirty = true;
            }
            SmokePhase::MetPrepare => {
                if let Some(rect) = probe(ctx, "met_reference_preview") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::MetQuoted;
                }
            }
            SmokePhase::MetQuoted => {
                if self.met_reference.quote.is_some()
                    && let Some(rect) = probe(ctx, "met_reference_commit")
                {
                    let path = smoke.output_dir.join("met-reminder-mapping-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::MetSaved;
                }
            }
            SmokePhase::MetSaved => {
                let lib = self.library.as_ref().unwrap();
                let id = self.selected_id.as_ref().unwrap();
                if lib
                    .met_reference_context(id, Role::Storyteller)
                    .is_ok_and(|c| c.history.len() == 1)
                {
                    let doc = lib
                        .print_document(
                            id,
                            &grapevine_workbench::printing::Options {
                                include_simple_mechanics: true,
                                ..Default::default()
                            },
                            lib.revision().unwrap(),
                            Role::Player,
                        )
                        .unwrap();
                    let context = lib.met_reference_context(id, Role::Storyteller).unwrap();
                    let valid = context.traits[1]["Name"] == "Synthetic second"
                        && context.traits[1]["Value"] == "basic"
                        && doc
                            .sections
                            .iter()
                            .filter(|s| s.title.starts_with("Simple mechanics:"))
                            .count()
                            == 2;
                    let privacy = !serde_json::to_string(&doc)
                        .unwrap()
                        .contains("PRIVATE_MET_NATIVE");
                    let no_grant = lib
                        .xp_view(id, Role::Storyteller)
                        .is_ok_and(|v| v.ledger.is_none())
                        && lib
                            .purchase_receipts(id, Role::Storyteller)
                            .is_ok_and(|r| r.is_empty());
                    smoke.checks.insert(
                        "met_reference_native_review_and_print".into(),
                        serde_json::json!(valid),
                    );
                    smoke.checks.insert(
                        "met_reference_native_privacy_no_grant".into(),
                        serde_json::json!(privacy && no_grant),
                    );
                    smoke.phase = if valid && privacy && no_grant {
                        SmokePhase::LifecycleOpen
                    } else {
                        smoke.failure = Some(
                            "MET reminder mapping changed the trait or leaked review evidence"
                                .into(),
                        );
                        SmokePhase::Capture
                    };
                }
            }
            SmokePhase::LifecycleOpen => {
                smoke.phase = SmokePhase::LifecyclePrepare;
                self.met_reference.open = false;
                self.open_character_lifecycle();
                self.lifecycle.request = grapevine_workbench::character_lifecycle::Request {
                    action: grapevine_workbench::character_lifecycle::Action::Retire,
                    day: "2026-11-14".into(),
                    actor: "PRIVATE_LIFECYCLE_NATIVE_ACTOR".into(),
                    reason: "PRIVATE_LIFECYCLE_NATIVE_REASON".into(),
                    review_reference: "PRIVATE_LIFECYCLE_NATIVE_REVIEW".into(),
                    confirmed: true,
                    ..Default::default()
                };
                self.lifecycle.dirty = true;
            }
            SmokePhase::LifecyclePrepare => {
                if let Some(rect) = probe(ctx, "lifecycle_preview") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::LifecycleQuoted;
                }
            }
            SmokePhase::LifecycleQuoted => {
                if self.lifecycle.quote.is_some()
                    && let Some(rect) = probe(ctx, "lifecycle_commit")
                {
                    let path = smoke.output_dir.join("lifecycle-review-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::LifecycleSaved;
                }
            }
            SmokePhase::LifecycleSaved => {
                let lib = self.library.as_ref().unwrap();
                let id = self.selected_id.as_ref().unwrap();
                if let Ok(context) = lib.character_lifecycle_context(id, Role::Storyteller)
                    && context.history.len() == 1
                {
                    let doc = lib
                        .print_document(
                            id,
                            &Default::default(),
                            lib.revision().unwrap(),
                            Role::Player,
                        )
                        .unwrap();
                    let valid = context.status == "Retired" && !context.status_drift
                        && self.selected_record.as_ref().is_some_and(|r|r.status=="Retired")
                        && lib.sheet_edit(id,Role::Storyteller).unwrap().lists["DisciplineList"]["Traits"].as_array().unwrap().len()==3;
                    let private = !serde_json::to_string(&doc)
                        .unwrap()
                        .contains("PRIVATE_LIFECYCLE_NATIVE")
                        && lib
                            .xp_view(id, Role::Storyteller)
                            .is_ok_and(|v| v.ledger.is_none());
                    smoke.checks.insert(
                        "lifecycle_native_review_status".into(),
                        serde_json::json!(valid),
                    );
                    smoke.checks.insert(
                        "lifecycle_native_privacy_no_xp_change".into(),
                        serde_json::json!(private),
                    );
                    smoke.phase = if valid && private {
                        SmokePhase::MixedSetup
                    } else {
                        smoke.failure=Some("Lifecycle save changed traits/XP, failed to refresh status or exposed private review".into());
                        SmokePhase::Capture
                    };
                }
            }
            SmokePhase::MixedSetup => {
                let path = smoke.output_dir.join("mixed-only.gvlib");
                smoke.phase = SmokePhase::MixedPreview;
                let (id, requests) = smoke_mixed_fixture(&path);
                self.route = WorkArea::Storyteller;
                self.open_path(path);
                self.select_record(id.clone());
                let lib = self.library.as_ref().unwrap();
                let q = lib
                    .quote_mixed_advancement(
                        &id,
                        &requests,
                        lib.revision().unwrap(),
                        Role::Storyteller,
                    )
                    .unwrap();
                for request in requests {
                    assert!(self.queue_mixed_request(id.clone(), request, q.record_hash.clone()));
                }
            }
            SmokePhase::MixedPreview => {
                if let Some(rect) = probe(ctx, "mixed_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::MixedSaved;
                }
            }
            SmokePhase::MixedSaved => {
                if let Some(q) = &self.mixed.quote {
                    let valid =
                        q.total_spend == "7" && q.before_unspent == "20" && q.after_unspent == "13";
                    smoke.checks.insert(
                        "mixed_native_ordered_preview".into(),
                        serde_json::json!(valid),
                    );
                    if !valid {
                        smoke.failure = Some(
                            "Mixed cumulative XP preview differs from supported source prices"
                                .into(),
                        );
                        smoke.phase = SmokePhase::Capture;
                    } else if let Some(rect) = probe(ctx, "mixed_save_apply") {
                        let path = smoke.output_dir.join("mixed-plan-preview-smoke.bmp");
                        ctx.request_screenshot(move |image| {
                            let _ = write_bmp(&path, &image);
                        });
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::MixedApply;
                    }
                }
            }
            SmokePhase::MixedApply => {
                if self.route == WorkArea::Builder
                    && let Some(review) = &self.mixed.review
                {
                    let lib = self.library.as_ref().unwrap();
                    let untouched = lib
                        .xp_view(&review.quote.record_id, Role::Storyteller)
                        .unwrap()
                        .unspent
                        .as_deref()
                        == Some("20");
                    smoke.checks.insert(
                        "mixed_native_draft_no_live_spend".into(),
                        serde_json::json!(untouched),
                    );
                    if !untouched {
                        smoke.failure = Some("Saving a mixed Draft spent live XP".into());
                        smoke.phase = SmokePhase::Capture;
                    } else if let Some(rect) = probe(ctx, "mixed_save_apply") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::MixedApplied;
                    }
                }
            }
            SmokePhase::MixedApplied => {
                if let Some(review) = &self.mixed.review
                    && review.applied
                {
                    let lib = self.library.as_ref().unwrap();
                    let id = &review.quote.record_id;
                    let valid = lib
                        .xp_view(id, Role::Storyteller)
                        .unwrap()
                        .unspent
                        .as_deref()
                        == Some("13")
                        && lib.sheet_edit(id, Role::Storyteller).unwrap().lists["AbilityList"]["Traits"]
                            [0]["Value"]
                            == "6"
                        && lib
                            .mixed_advancement_history(id, Role::Storyteller)
                            .unwrap()
                            .len()
                            == 1
                        && lib.purchase_receipts(id, Role::Storyteller).unwrap().len() == 1;
                    let mut privacy = true;
                    for (label, language) in [
                        ("en", grapevine_workbench::printing::DocLanguage::English),
                        ("pt", grapevine_workbench::printing::DocLanguage::Portuguese),
                    ] {
                        let doc = lib
                            .print_document(
                                id,
                                &grapevine_workbench::printing::Options {
                                    language,
                                    include_simple_mechanics: true,
                                    ..Default::default()
                                },
                                lib.revision().unwrap(),
                                Role::Player,
                            )
                            .unwrap();
                        let html = grapevine_workbench::printing::html(&doc).unwrap();
                        privacy &= !html.contains("PRIVATE_MIXED");
                        std::fs::write(smoke.output_dir.join(format!("mixed-{label}.html")), html)
                            .unwrap();
                    }
                    smoke.checks.insert(
                        "mixed_native_apply_receipts_traits".into(),
                        serde_json::json!(valid),
                    );
                    smoke.checks.insert(
                        "mixed_native_print_privacy".into(),
                        serde_json::json!(privacy),
                    );
                    smoke.phase = if valid && privacy {
                        SmokePhase::LegacyProfileSetup
                    } else {
                        smoke.failure = Some("Mixed apply/receipt/printing result failed".into());
                        SmokePhase::Capture
                    };
                }
            }
            SmokePhase::LegacyProfileSetup => {
                let path = smoke.output_dir.join("legacy-profile-only.gvlib");
                smoke.phase = SmokePhase::LegacyProfilePreview;
                smoke_legacy_profile_fixture(&path);
                self.route = WorkArea::Storyteller;
                self.open_path(path);
                self.open_legacy_profile();
                self.legacy_profile.request = smoke_legacy_profile_request(
                    self.legacy_profile
                        .context
                        .as_ref()
                        .unwrap()
                        .source_hash
                        .clone(),
                );
                self.legacy_profile.dirty = true;
            }
            SmokePhase::LegacyProfilePreview => {
                if let Some(rect) = probe(ctx, "legacy_profile_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::LegacyProfileApply;
                }
            }
            SmokePhase::LegacyProfileApply => {
                if let Some(q) = &self.legacy_profile.quote {
                    let valid = q.changes.len() == 7
                        && !q.proposed.publish_contacts
                        && !q.proposed.publish_details;
                    smoke.checks.insert(
                        "legacy_profile_native_review_choices".into(),
                        serde_json::json!(valid),
                    );
                    if !valid {
                        smoke.failure = Some(
                            "Legacy profile preview changed publication settings or lost fields"
                                .into(),
                        );
                        smoke.phase = SmokePhase::Capture;
                    } else if let Some(rect) = probe(ctx, "legacy_profile_apply") {
                        let path = smoke.output_dir.join("legacy-profile-review-smoke.bmp");
                        ctx.request_screenshot(move |image| {
                            let _ = write_bmp(&path, &image);
                        });
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::LegacyProfileApplied;
                    }
                }
            }
            SmokePhase::LegacyProfileApplied => {
                if let Some(c) = &self.legacy_profile.context
                    && c.history.len() == 1
                {
                    let lib = self.library.as_ref().unwrap();
                    let public = lib.public_chronicle_profile().unwrap();
                    let valid = c.profile.title == "Synthetic legacy chronicle"
                        && c.profile.email == "staff@example.invalid"
                        && lib.title == c.profile.title
                        && c.profile.house_rules == "Local rules retained";
                    let private = public.profile.email.is_empty()
                        && public.profile.website.is_empty()
                        && !serde_json::to_string(&public)
                            .unwrap()
                            .contains("PRIVATE_LEGACY");
                    smoke.checks.insert(
                        "legacy_profile_native_atomic_title_details".into(),
                        serde_json::json!(valid),
                    );
                    smoke.checks.insert(
                        "legacy_profile_native_publication_privacy".into(),
                        serde_json::json!(private),
                    );
                    smoke.phase = if valid && private {
                        SmokePhase::LegacyCalendarSetup
                    } else {
                        smoke.failure =
                            Some("Legacy profile apply/public projection failed".into());
                        SmokePhase::Capture
                    };
                }
            }
            SmokePhase::LegacyCalendarSetup => {
                let path = smoke.output_dir.join("legacy-calendar-only.gvlib");
                smoke_legacy_calendar_fixture(&path);
                smoke.phase = SmokePhase::LegacyCalendarPreview;
                self.route = WorkArea::Storyteller;
                self.open_path(path);
                self.open_legacy_calendar();
                let hash = self
                    .legacy_calendar
                    .context
                    .as_ref()
                    .unwrap()
                    .source_hash
                    .clone();
                self.legacy_calendar.request = smoke_legacy_calendar_request(hash);
                for (selected, choice) in &mut self.legacy_calendar.rows {
                    if let Some(requested) = self
                        .legacy_calendar
                        .request
                        .choices
                        .iter()
                        .find(|c| c.entry_index == choice.entry_index)
                    {
                        *selected = true;
                        *choice = requested.clone();
                    }
                }
                self.legacy_calendar.dirty = true;
            }
            SmokePhase::LegacyCalendarPreview => {
                if let Some(rect) = probe(ctx, "legacy_calendar_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::LegacyCalendarApply;
                }
            }
            SmokePhase::LegacyCalendarApply => {
                if let Some(q) = &self.legacy_calendar.quote {
                    let valid = q.plans.len() == 2
                        && q.plans.iter().all(|p| p.request.attendance.is_empty())
                        && q.plans[0].request.day == q.plans[1].request.day;
                    smoke.checks.insert(
                        "legacy_calendar_native_review_no_awards".into(),
                        serde_json::json!(valid),
                    );
                    if !valid {
                        smoke.failure = Some(
                            "Legacy calendar preview inferred attendance/awards or merged entries"
                                .into(),
                        );
                        smoke.phase = SmokePhase::Capture;
                    } else if let Some(rect) = probe(ctx, "legacy_calendar_apply") {
                        let path = smoke.output_dir.join("legacy-calendar-review-smoke.bmp");
                        ctx.request_screenshot(move |image| {
                            let _ = write_bmp(&path, &image);
                        });
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::LegacyCalendarApplied;
                    }
                }
            }
            SmokePhase::LegacyCalendarApplied => {
                if let Some(c) = &self.legacy_calendar.context
                    && c.history.len() == 1
                {
                    let lib = self.library.as_ref().unwrap();
                    let saved = lib.scheduled_sessions(Role::Storyteller).unwrap();
                    let source = lib
                        .summaries(Role::Storyteller)
                        .unwrap()
                        .into_iter()
                        .find(|s| s.kind == "Vampire")
                        .unwrap();
                    let unchanged = lib
                        .xp_view(&source.id, Role::Storyteller)
                        .unwrap()
                        .unspent
                        .as_deref()
                        == Some("10")
                        && lib.session_receipts(Role::Storyteller).unwrap().is_empty();
                    let separate = saved.len() == 2
                        && saved[0].request.id != saved[1].request.id
                        && saved.iter().all(|s| s.request.attendance.is_empty());
                    let public =
                        serde_json::to_string(&lib.public_chronicle_profile().unwrap()).unwrap();
                    let private = !public.contains("PRIVATE_CALENDAR")
                        && !public.contains(grapevine_workbench::legacy_calendar::FIELD);
                    smoke.checks.insert(
                        "legacy_calendar_native_atomic_private".into(),
                        serde_json::json!(unchanged && separate && private),
                    );
                    if unchanged && separate && private {
                        smoke.phase = SmokePhase::LegacyCalendarLoad;
                        self.legacy_calendar.open = false;
                        self.open_sessions();
                    } else {
                        smoke.failure=Some("Legacy calendar apply changed XP, merged plans or exposed private history".into());
                        smoke.phase = SmokePhase::Capture;
                    }
                }
            }
            SmokePhase::LegacyCalendarLoad => {
                if let Some(rect) = probe(ctx, "session_load0") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::LegacyCalendarLoaded;
                }
            }
            SmokePhase::LegacyCalendarLoaded => {
                let valid = self.session_form.schedules.first().is_some_and(|s| {
                    s.request.id == self.session_form.request.id
                        && s.request.title == self.session_form.request.title
                }) && self
                    .session_form
                    .request
                    .title
                    .starts_with("Synthetic imported date ")
                    && self.session_form.legacy_origin.as_ref().is_some_and(|o| {
                        o.imported_day == "2026-12-01"
                            && o.entry.time == if o.entry.index == 0 { "7pm" } else { "later" }
                    })
                    && self.session_form.request.attendance.is_empty()
                    && self
                        .session_form
                        .rows
                        .values()
                        .all(|r| !r.included && !r.present && !r.award_enabled);
                if valid {
                    smoke.checks.insert(
                        "legacy_calendar_native_load_unattended".into(),
                        serde_json::json!(true),
                    );
                    self.session_open = false;
                    self.session_form = Default::default();
                    smoke.phase = SmokePhase::BatchDone;
                }
            }
            SmokePhase::BatchDone => {
                let path = smoke.library_path.clone();
                let selected = smoke.batch_previous_selection.take();
                smoke.phase = SmokePhase::SessionClick;
                self.route = WorkArea::Storyteller;
                self.open_path(path);
                if let Some(id) = selected {
                    self.selected_id = Some(id.clone());
                    self.load_record(&id, false);
                }
                self.language = Language::English;
            }
            SmokePhase::SessionClick => {
                self.session_form = crate::session_ui::Form::from_roster(&self.summaries);
                if let Some(rect) = probe(ctx, "sessions_open") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::SessionOpen;
                } else {
                    smoke.failure = Some("Sessions toolbar control was not visible".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::SessionOpen => {
                if self.session_open {
                    let request = &mut self.session_form.request;
                    request.title = "Synthetic session".into();
                    request.day = "2026-01-02".into();
                    request.actor = "Synthetic ST".into();
                    request.reason = "Manual test award".into();
                    request.evidence = "Synthetic attendance".into();
                    let risen = self
                        .summaries
                        .iter()
                        .find(|s| s.kind == "Vampire" && s.build.is_none())
                        .unwrap();
                    let row = self.session_form.rows.get_mut(&risen.id).unwrap();
                    row.included = true;
                    row.present = true;
                    row.award_enabled = true;
                    row.amount = "2".into();
                    self.session_form.sync_request(&self.summaries);
                    self.session_form.dirty = true;
                    smoke.phase = SmokePhase::SessionPrepared;
                }
            }
            SmokePhase::SessionPrepared => {
                if let Some(rect) = probe(ctx, "session_schedule") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::SessionScheduled;
                }
            }
            SmokePhase::SessionScheduled => {
                if self.session_form.schedules.len() == 1 {
                    let lib = Library::open(&smoke.library_path).unwrap();
                    let saved = lib.scheduled_sessions(Role::Storyteller).unwrap();
                    let id = &saved[0].request.attendance[0].record_id;
                    let unchanged = lib
                        .xp_view(id, Role::Storyteller)
                        .unwrap()
                        .unspent
                        .as_deref()
                        == Some("10.25");
                    smoke.checks.insert(
                        "calendar_native_save_reopen_no_xp".into(),
                        serde_json::json!(saved.len() == 1 && unchanged),
                    );
                    if let Some(rect) = probe(ctx, "session_load0") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::SessionPreviewReady;
                    }
                } else if let Some(message) = &self.session_form.message {
                    smoke.failure = Some(format!("Scheduled event save failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::SessionPreviewReady => {
                if let Some(rect) = probe(ctx, "session_preview") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::SessionPreviewed;
                }
            }
            SmokePhase::SessionPreviewed => {
                if let Some(q) = &self.session_form.quote {
                    let ok = q.awards.len() == 1
                        && q.awards[0].before_unspent == "10.25"
                        && q.awards[0].after_unspent == "12.25";
                    smoke.checks.insert(
                        "session_native_manual_preview".into(),
                        serde_json::json!(ok),
                    );
                    if !ok {
                        smoke.failure = Some("Session preview did not show10.25to12.25".into());
                        smoke.phase = SmokePhase::Capture;
                    } else if let Some(rect) = probe(ctx, "session_commit") {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::SessionCommitted;
                    }
                } else if let Some(message) = &self.session_form.message {
                    smoke.failure = Some(format!("Synthetic session preview failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::SessionCommitted => {
                if !self.session_form.receipts.is_empty() && self.session_form.quote.is_none() {
                    let receipt = &self.session_form.receipts[0];
                    let lib = self.library.as_ref().unwrap();
                    let html = lib
                        .session_report(
                            &receipt.quote.request.id,
                            Language::English,
                            Role::Storyteller,
                        )
                        .unwrap();
                    let path = smoke.output_dir.join("session-receipt.html");
                    grapevine_workbench::safe_files::write_new(&path, html.as_bytes()).unwrap();
                    let reopened = Library::open(&smoke.library_path).unwrap();
                    let saved = reopened.session_receipts(Role::Storyteller).unwrap();
                    let ok = saved.len() == 1
                        && saved[0].quote.awards[0].after_unspent == "12.25"
                        && reopened
                            .scheduled_sessions(Role::Storyteller)
                            .unwrap()
                            .is_empty();
                    smoke.checks.insert(
                        "session_native_commit_reopen_and_report".into(),
                        serde_json::json!(ok && path.is_file()),
                    );
                    let path = smoke.output_dir.join("session-receipt-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::SessionDone;
                }
            }
            SmokePhase::SessionDone => {
                if smoke.output_dir.join("session-receipt-smoke.bmp").is_file() {
                    self.session_open = false;
                    // Editing after the retained receipt must ask before navigation, then clear private state.
                    self.session_form.request.title = "Unposted synthetic draft".into();
                    self.session_form.dirty = true;
                    self.pending_action = Some(PendingAction::Route(WorkArea::Builder));
                    self.session_form.dirty = false;
                    self.pending_action = None;
                    smoke.phase = SmokePhase::PointsClick;
                }
            }
            SmokePhase::PointsClick => {
                if let Some(rect) = probe(ctx, "points_open") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PointsOpen;
                }
            }
            SmokePhase::PointsOpen => {
                if self.points_open {
                    self.points_form.name = "Synthetic Player".into();
                    self.points_form.dirty = true;
                    smoke.phase = SmokePhase::PointsCreatePrepared;
                }
            }
            SmokePhase::PointsCreatePrepared => {
                if let Some(rect) = probe(ctx, "points_create") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PointsCreated;
                }
            }
            SmokePhase::PointsCreated => {
                if self.points_target.is_some() {
                    self.points_form.day = "2026-01-01".into();
                    self.points_form.actor = "Synthetic ST".into();
                    self.points_form.reason = "Synthetic opening".into();
                    self.points_form.evidence = "Synthetic fixture".into();
                    self.points_form.opening_balance = "5.25".into();
                    self.points_form.dirty = true;
                    smoke.phase = SmokePhase::PointsOpeningPrepared;
                }
            }
            SmokePhase::PointsOpeningPrepared => {
                if let Some(rect) = probe(ctx, "points_opening") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PointsOpened;
                }
            }
            SmokePhase::PointsOpened => {
                if self.points_ledger.is_some() {
                    self.points_form.day = "2026-01-02".into();
                    self.points_form.reason = "Synthetic manual award".into();
                    self.points_form.evidence = "Synthetic fixture".into();
                    self.points_form.amount = "2".into();
                    self.points_form.kind = grapevine_workbench::player_points::Kind::Award;
                    self.points_form.dirty = true;
                    smoke.phase = SmokePhase::PointsAwardPrepared;
                } else if let Some(message) = &self.points_form.message {
                    smoke.failure = Some(format!("Player points opening failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::PointsAwardPrepared => {
                if let Some(rect) = probe(ctx, "points_post") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PointsAwarded;
                }
            }
            SmokePhase::PointsAwarded => {
                if self
                    .points_ledger
                    .as_ref()
                    .is_some_and(|l| l.entries.len() == 1)
                {
                    let ok = grapevine_workbench::player_points::replay(
                        self.points_ledger.as_ref().unwrap(),
                    )
                    .unwrap()
                        == "7.25";
                    smoke.checks.insert(
                        "player_points_native_open_and_award".into(),
                        serde_json::json!(ok),
                    );
                    let reopened = Library::open(&smoke.library_path).unwrap();
                    let ledger = reopened
                        .player_points(self.points_target.as_ref().unwrap(), Role::Storyteller)
                        .unwrap()
                        .unwrap();
                    let vampire = self
                        .summaries
                        .iter()
                        .find(|s| s.kind == "Vampire" && s.build.is_none())
                        .unwrap();
                    let xp = reopened.xp_view(&vampire.id, Role::Storyteller).unwrap();
                    smoke.checks.insert(
                        "player_points_reopen_and_separate_character_xp".into(),
                        serde_json::json!(
                            grapevine_workbench::player_points::replay(&ledger).unwrap() == "7.25"
                                && xp.unspent.as_deref() == Some("12.25")
                        ),
                    );
                    let path = smoke.output_dir.join("player-points-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::PointsDone;
                } else if let Some(message) = &self.points_form.message {
                    smoke.failure = Some(format!("Player points award failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::PointsDone => {
                if smoke.output_dir.join("player-points-smoke.bmp").is_file() {
                    self.points_open = false;
                    smoke.phase = SmokePhase::WorldPrepare;
                }
            }
            SmokePhase::WorldPrepare => {
                self.world = world_window::WorldState::synthetic_draft(
                    self.library.as_ref().and_then(|lib| lib.revision().ok()),
                );
                self.world.edit.name = "Synthetic world item".into();
                self.world
                    .edit
                    .fields
                    .insert("Powers".into(), "Synthetic descriptive effect".into());
                self.world.edit.fields.insert("Bonus".into(), 2.into());
                smoke.phase = SmokePhase::WorldSave;
            }
            SmokePhase::WorldSave => {
                if let Some(rect) = probe(ctx, "world_save") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::WorldCheck;
                }
            }
            SmokePhase::WorldCheck => {
                if let Some(id) = self.world.target.clone() {
                    let reopened = Library::open(&smoke.library_path).unwrap();
                    let edit = reopened.world_edit(&id, Role::Storyteller).unwrap();
                    smoke.checks.insert(
                        "world_item_native_save_reopen".into(),
                        serde_json::json!(
                            edit.name == "Synthetic world item"
                                && edit.fields["Bonus"] == 2
                                && !self.world.dirty()
                        ),
                    );
                    smoke.checks.insert(
                        "world_export_control_visible".into(),
                        serde_json::json!(probe(ctx, "world_export").is_some()),
                    );
                    self.world.edit.name = "Unposted world edit".into();
                    self.world.open = false;
                    self.language = Language::Portuguese;
                    self.world.open = true;
                    smoke.checks.insert(
                        "world_draft_survives_close_language".into(),
                        serde_json::json!(
                            self.world.edit.name == "Unposted world edit" && self.world.dirty()
                        ),
                    );
                    self.world.edit.name = "Synthetic world item".into();
                    let path = smoke.output_dir.join("world-item-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::WorldDone;
                } else if let Some(message) = &self.world.message {
                    smoke.failure = Some(format!("World save failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::WorldDone => {
                if smoke.output_dir.join("world-item-smoke.bmp").is_file() {
                    self.world.open = false;
                    smoke.phase = SmokePhase::PlotPrepare;
                }
            }
            SmokePhase::PlotPrepare => {
                self.plot = plot_window::PlotWindow::synthetic(
                    self.library.as_ref().and_then(|lib| lib.revision().ok()),
                );
                smoke.phase = SmokePhase::PlotSave;
            }
            SmokePhase::PlotSave => {
                if let Some(rect) = probe(ctx, "plot_save") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::PlotCheck;
                }
            }
            SmokePhase::PlotCheck => {
                if let Some(id) = self.plot.target.clone() {
                    let reopened = Library::open(&smoke.library_path).unwrap();
                    let snapshot = reopened.plot_snapshot(&id, Role::Storyteller).unwrap();
                    let html = grapevine_workbench::plots::html(
                        &snapshot,
                        "2026-11-10",
                        Language::Portuguese,
                        false,
                    )
                    .unwrap();
                    smoke.checks.insert(
                        "plot_native_save_and_reopen".into(),
                        serde_json::json!(
                            snapshot.plot.name == "Synthetic plot"
                                && snapshot.plot.developments.len() == 2
                                && !self.plot.dirty()
                        ),
                    );
                    smoke.checks.insert(
                        "plot_private_and_report_visible".into(),
                        serde_json::json!(
                            reopened.record(&id, Role::Player).is_err()
                                && probe(ctx, "plot_report").is_some()
                                && html.contains("Relatório privado")
                                && !html.contains("SYNTHETIC_CAST_NOTE")
                                && html.contains("&lt;literal&gt;")
                        ),
                    );
                    let _ = std::fs::write(smoke.output_dir.join("private-plot.html"), html);
                    let path = smoke.output_dir.join("plot-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::PlotDone;
                } else if let Some(message) = &self.plot.message {
                    smoke.failure = Some(format!("Plot save failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::PlotDone => {
                if smoke.output_dir.join("plot-smoke.bmp").is_file() {
                    self.plot.open = false;
                    smoke.phase =
                        SmokePhase::StoryPrepare(grapevine_workbench::story_work::Kind::Action);
                }
            }
            SmokePhase::StoryPrepare(kind) => {
                self.story = story_window::StoryWindow::synthetic(
                    kind,
                    self.library.as_ref().and_then(|lib| lib.revision().ok()),
                );
                smoke.phase = SmokePhase::StorySave(kind);
            }
            SmokePhase::StorySave(kind) => {
                if let Some(rect) = probe(ctx, "story_save") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::StoryCheck(kind);
                }
            }
            SmokePhase::StoryCheck(kind) => {
                if let Some(id) = self.story.target.clone() {
                    let reopened = Library::open(&smoke.library_path).unwrap();
                    let s = reopened.story_snapshot(&id, Role::Storyteller).unwrap();
                    let master = reopened
                        .master_story(kind, "2026-11-01", "2026-11-30", false, Role::Storyteller)
                        .unwrap();
                    let html =
                        grapevine_workbench::story_work::master_html(&master, Language::Portuguese)
                            .unwrap();
                    let key = kind.record_kind().to_ascii_lowercase();
                    smoke.checks.insert(
                        format!("{key}_native_save_reopen_private"),
                        serde_json::json!(
                            s.draft.kind() == kind
                                && s.draft.ready()
                                && !self.story.dirty()
                                && reopened.record(&id, Role::Player).is_err()
                        ),
                    );
                    smoke.checks.insert(
                        format!("{key}_native_report_and_scope"),
                        serde_json::json!(
                            master.records.len() == 1
                                && probe(ctx, "story_report").is_some()
                                && html.contains("Relatório privado")
                                && html.contains("&lt;literal&gt;")
                        ),
                    );
                    let _ =
                        std::fs::write(smoke.output_dir.join(format!("private-{key}.html")), html);
                    let path = smoke.output_dir.join(format!("{key}-smoke.bmp"));
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::StoryDone(kind);
                } else if let Some(msg) = &self.story.message {
                    smoke.failure = Some(format!("Story save failed: {msg}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::StoryDone(kind) => {
                let key = kind.record_kind().to_ascii_lowercase();
                if smoke.output_dir.join(format!("{key}-smoke.bmp")).is_file() {
                    self.story.open = false;
                    smoke.phase = match kind {
                        grapevine_workbench::story_work::Kind::Action => {
                            SmokePhase::StoryPrepare(grapevine_workbench::story_work::Kind::Rumor)
                        }
                        grapevine_workbench::story_work::Kind::Rumor => {
                            SmokePhase::RumorAudiencePrepare
                        }
                    };
                }
            }
            SmokePhase::RumorAudiencePrepare => {
                self.rumor_audience = Default::default();
                self.rumor_audience.open = true;
                self.rumor_audience.from = "2026-11-01".into();
                self.rumor_audience.through = "2026-11-30".into();
                smoke.phase = SmokePhase::RumorAudienceQueue;
            }
            SmokePhase::RumorAudienceQueue => {
                if let Some(rect) = probe(ctx, "rumor_audience_queue") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::RumorAudienceQueueCheck;
                }
            }
            SmokePhase::RumorAudienceQueueCheck => {
                if let Some(queue) = &self.rumor_audience.queue {
                    let lib = self.library.as_ref().unwrap();
                    smoke.checks.insert(
                        "rumor_audience_native_queue_readonly".into(),
                        serde_json::json!(
                            queue.rows.len() == 1
                                && queue.revision == lib.revision().unwrap()
                                && lib
                                    .rumor_audience_queue("2026-11-01", "2026-11-30", Role::Player)
                                    .is_err()
                        ),
                    );
                    let id = self.story.target.as_ref().unwrap();
                    let snapshot = lib.rumor_audience_snapshot(id, Role::Storyteller).unwrap();
                    let character = self
                        .summaries
                        .iter()
                        .find(|r| {
                            r.build.is_none()
                                && grapevine_workbench::sheet::is_character_kind(&r.kind)
                        })
                        .unwrap();
                    self.rumor_audience = rumor_audience_window::RumorAudienceWindow::synthetic(
                        snapshot,
                        character.id.clone(),
                    );
                    smoke.phase = SmokePhase::RumorAudiencePreview;
                } else if let Some(message) = &self.rumor_audience.message {
                    smoke.failure = Some(format!("Rumor audience queue failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::RumorAudiencePreview => {
                if let Some(rect) = probe(ctx, "rumor_audience_preview") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::RumorAudiencePreviewCheck;
                }
            }
            SmokePhase::RumorAudiencePreviewCheck => {
                if let Some(preview) = &self.rumor_audience.preview {
                    let lib = self.library.as_ref().unwrap();
                    smoke.checks.insert(
                        "rumor_audience_native_preview_readonly".into(),
                        serde_json::json!(
                            preview.recipients.len() == 1
                                && preview.revision == lib.revision().unwrap()
                                && lib
                                    .rumor_audience_snapshot(&preview.rumor_id, Role::Storyteller)
                                    .unwrap()
                                    .recipients
                                    .is_empty()
                        ),
                    );
                    smoke.phase = SmokePhase::RumorAudienceSave;
                } else if let Some(message) = &self.rumor_audience.message {
                    smoke.failure = Some(format!("Rumor audience preview failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::RumorAudienceSave => {
                if let Some(rect) = probe(ctx, "rumor_audience_save") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::RumorAudienceCheck;
                }
            }
            SmokePhase::RumorAudienceCheck => {
                if !self.rumor_audience.dirty() {
                    let reopened = Library::open(&smoke.library_path).unwrap();
                    let id = self.story.target.as_ref().unwrap();
                    let snapshot = reopened
                        .rumor_audience_snapshot(id, Role::Storyteller)
                        .unwrap();
                    let html =
                        grapevine_workbench::rumor_packet::html(&snapshot, Language::Portuguese)
                            .unwrap();
                    std::fs::write(smoke.output_dir.join("rumor-audience.html"), &html).unwrap();
                    smoke.checks.insert(
                        "rumor_audience_native_save_reopen_private".into(),
                        serde_json::json!(
                            snapshot.recipients.len() == 1
                                && !html.contains("SMOKE_ONLY_SECRET")
                                && !html.contains("PRIVATE_PROFILE_EMAIL")
                                && self.story.revision == Some(snapshot.revision)
                                && reopened.rumor_audience_snapshot(id, Role::Player).is_err()
                        ),
                    );
                    let path = smoke.output_dir.join("rumor-audience-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::RumorAudienceDone;
                } else if let Some(message) = &self.rumor_audience.message {
                    smoke.failure = Some(format!("Rumor audience save failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::RumorAudienceDone => {
                if smoke.output_dir.join("rumor-audience-smoke.bmp").is_file() {
                    self.rumor_audience.open = false;
                    smoke.phase = SmokePhase::AssociationPrepare;
                }
            }
            SmokePhase::AssociationPrepare => {
                let character = self.summaries.iter().find(|r| {
                    r.build.is_none() && grapevine_workbench::sheet::is_character_kind(&r.kind)
                });
                let target = self
                    .summaries
                    .iter()
                    .find(|r| r.kind == "Item" && r.name == "Synthetic world item");
                if let (Some(lib), Some(c), Some(target)) = (&self.library, character, target) {
                    let snapshot = lib.association_snapshot(&c.id, Role::Storyteller).unwrap();
                    self.associations =
                        association_window::AssociationWindow::synthetic(snapshot, target.clone());
                    smoke.phase = SmokePhase::AssociationSave;
                } else {
                    smoke.failure = Some("Synthetic association fixture records missing".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::AssociationSave => {
                if let Some(rect) = probe(ctx, "association_save") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::AssociationCheck;
                }
            }
            SmokePhase::AssociationCheck => {
                if !self.associations.dirty() {
                    let id = self.associations.character_id().unwrap();
                    let reopened = Library::open(&smoke.library_path).unwrap();
                    let s = reopened
                        .association_snapshot(id, Role::Storyteller)
                        .unwrap();
                    let html =
                        grapevine_workbench::associations::html(&s, Language::Portuguese, false)
                            .unwrap();
                    smoke.checks.insert(
                        "association_native_save_reopen_private".into(),
                        serde_json::json!(
                            s.linked.len() == 1
                                && s.linked[0].link.quantity == 2
                                && s.linked[0]
                                    .current
                                    .as_ref()
                                    .is_some_and(|e| e.name == "Synthetic world item")
                                && reopened.association_snapshot(id, Role::Player).is_err()
                        ),
                    );
                    smoke.checks.insert(
                        "association_native_report_visible".into(),
                        serde_json::json!(
                            probe(ctx, "association_report").is_some()
                                && html.contains("Synthetic world item")
                                && !html.contains("SYNTHETIC_ASSOCIATION_NOTE")
                        ),
                    );
                    let _ = std::fs::write(smoke.output_dir.join("private-equipment.html"), html);
                    let path = smoke.output_dir.join("equipment-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::AssociationDone;
                } else if let Some(msg) = &self.associations.message {
                    smoke.failure = Some(format!("Association save failed: {msg}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::AssociationDone => {
                if smoke.output_dir.join("equipment-smoke.bmp").is_file() {
                    self.associations.open = false;
                    smoke.phase = SmokePhase::OperationsPrepare;
                }
            }
            SmokePhase::OperationsPrepare => {
                self.operations = operations_window::OperationsWindow::synthetic();
                smoke.phase = SmokePhase::OperationsClick;
            }
            SmokePhase::OperationsClick => {
                if let Some(rect) = probe(ctx, "operations_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::OperationsCheck;
                }
            }
            SmokePhase::OperationsCheck => {
                if self.operations.is_prepared() {
                    use grapevine_workbench::operations;
                    let reopened = Library::open(&smoke.library_path).unwrap();
                    let request = operations::Request::Roster {
                        family: String::new(),
                        status: String::new(),
                        include_players: false,
                    };
                    let before = reopened.revision().unwrap();
                    let s = reopened
                        .operations_snapshot(&request, Role::Storyteller)
                        .unwrap();
                    let html = operations::html(&s, Language::Portuguese).unwrap();
                    smoke.checks.insert(
                        "operations_native_prepared_readonly".into(),
                        serde_json::json!(
                            reopened.revision().unwrap() == before
                                && reopened
                                    .operations_snapshot(&request, Role::Player)
                                    .is_err()
                                && html.contains("Relatório privado")
                                && html.contains("Smoke Edited Character")
                        ),
                    );
                    smoke.checks.insert(
                        "operations_native_export_visible".into(),
                        serde_json::json!(probe(ctx, "operations_export").is_some()),
                    );
                    let _ = std::fs::write(smoke.output_dir.join("private-roster.html"), html);
                    let path = smoke.output_dir.join("operations-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::OperationsDone;
                } else if let Some(msg) = &self.operations.message {
                    smoke.failure = Some(format!("Operations preparation failed: {msg}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::OperationsDone => {
                if smoke.output_dir.join("operations-smoke.bmp").is_file() {
                    self.operations.open = false;
                    smoke.phase = SmokePhase::HistoryPrepare(false);
                }
            }
            SmokePhase::HistoryPrepare(player) => {
                let lib = self.library.as_ref().unwrap();
                let selected = self.summaries.iter().find(|s| {
                    s.build.is_none()
                        && if player {
                            s.kind == "Player"
                        } else {
                            grapevine_workbench::sheet::is_character_kind(&s.kind)
                                && lib
                                    .xp_view(&s.id, Role::Storyteller)
                                    .is_ok_and(|v| v.ledger.is_some())
                        }
                });
                if let Some(selected) = selected {
                    self.operations = operations_window::OperationsWindow::synthetic_history(
                        selected.id.clone(),
                        player,
                    );
                    smoke.phase = SmokePhase::HistoryClick(player);
                } else {
                    smoke.failure = Some("History fixture not found".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::HistoryClick(player) => {
                if let Some(rect) = probe(ctx, "operations_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::HistoryCheck(player);
                }
            }
            SmokePhase::HistoryCheck(player) => {
                if self.operations.is_prepared() {
                    let lib = Library::open(&smoke.library_path).unwrap();
                    let selected = self
                        .summaries
                        .iter()
                        .find(|s| {
                            s.build.is_none()
                                && if player {
                                    s.kind == "Player"
                                } else {
                                    grapevine_workbench::sheet::is_character_kind(&s.kind)
                                        && lib
                                            .xp_view(&s.id, Role::Storyteller)
                                            .is_ok_and(|v| v.ledger.is_some())
                                }
                        })
                        .unwrap();
                    let req = if player {
                        grapevine_workbench::operations::Request::PlayerHistory {
                            record_id: selected.id.clone(),
                            from: "2026-01-01".into(),
                            through: "2026-12-31".into(),
                            include_reasons: false,
                        }
                    } else {
                        grapevine_workbench::operations::Request::CharacterHistory {
                            record_id: selected.id.clone(),
                            from: "2026-01-01".into(),
                            through: "2026-12-31".into(),
                            include_reasons: false,
                        }
                    };
                    let s = lib.operations_snapshot(&req, Role::Storyteller).unwrap();
                    let html =
                        grapevine_workbench::operations::html(&s, Language::Portuguese).unwrap();
                    let key = if player { "points" } else { "xp" };
                    smoke.checks.insert(
                        format!("{key}_history_native_private_prepare"),
                        serde_json::json!(
                            probe(ctx, "operations_export").is_some()
                                && lib.operations_snapshot(&req, Role::Player).is_err()
                                && !html.contains("Synthetic opening")
                                && !html.contains("SECRET")
                        ),
                    );
                    let _ =
                        std::fs::write(smoke.output_dir.join(format!("{key}-history.html")), html);
                    let path = smoke.output_dir.join(format!("{key}-history-smoke.bmp"));
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::HistoryDone(player);
                } else if let Some(msg) = &self.operations.message {
                    smoke.failure = Some(format!("History prepare failed: {msg}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::HistoryDone(player) => {
                let key = if player { "points" } else { "xp" };
                if smoke
                    .output_dir
                    .join(format!("{key}-history-smoke.bmp"))
                    .is_file()
                {
                    self.operations.open = false;
                    smoke.phase = if player {
                        SmokePhase::ProfilePrepare
                    } else {
                        SmokePhase::HistoryPrepare(true)
                    };
                }
            }
            SmokePhase::ProfilePrepare => {
                self.profiles = profile_window::ProfileWindow::synthetic(self.revision);
                smoke.phase = SmokePhase::ProfileClick;
            }
            SmokePhase::ProfileClick => {
                if let Some(rect) = probe(ctx, "profile_save") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ProfileCheck;
                }
            }
            SmokePhase::ProfileCheck => {
                if !self.profiles.dirty()
                    && let Some(id) = &self.profiles.target
                {
                    let reopened = Library::open(&smoke.library_path).unwrap();
                    let snapshot = reopened.player_profile(id, Role::Storyteller).unwrap();
                    let public =
                        serde_json::to_string(&reopened.record(id, Role::Player).unwrap()).unwrap();
                    smoke.checks.insert(
                        "profile_native_save_reopen".into(),
                        serde_json::json!(
                            snapshot.profile.name == "Synthetic Profile"
                                && snapshot.profile.email == "PRIVATE_PROFILE_EMAIL"
                                && snapshot.profile.notes == "PRIVATE_PROFILE_NOTES"
                        ),
                    );
                    smoke.checks.insert(
                        "profile_native_private".into(),
                        serde_json::json!(
                            !public.contains("PRIVATE_PROFILE_")
                                && reopened.player_profile(id, Role::Player).is_err()
                        ),
                    );
                    let path = smoke.output_dir.join("profile-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::ProfileDone;
                } else if let Some(message) = &self.profiles.message {
                    smoke.failure = Some(format!("Profile save failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::ProfileDone => {
                if smoke.output_dir.join("profile-smoke.bmp").is_file() {
                    self.profiles.open = false;
                    smoke.phase = SmokePhase::SearchPrepare;
                }
            }
            SmokePhase::SearchPrepare => {
                self.searches = search_window::SearchWindow::synthetic(self.revision);
                smoke.phase = SmokePhase::SearchSave;
            }
            SmokePhase::SearchSave => {
                if let Some(rect) = probe(ctx, "search_save") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::SearchSaved;
                }
            }
            SmokePhase::SearchSaved => {
                if !self.searches.dirty() && self.searches.saved_id().is_some() {
                    smoke.phase = SmokePhase::SearchRun;
                } else if let Some(message) = &self.searches.message {
                    smoke.failure = Some(format!("Search save failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::SearchRun => {
                if let Some(rect) = probe(ctx, "search_run") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::SearchCheck;
                }
            }
            SmokePhase::SearchCheck => {
                if let Some(snapshot) = self.searches.prepared() {
                    let reopened = Library::open(&smoke.library_path).unwrap();
                    let index = reopened.saved_searches(Role::Storyteller).unwrap();
                    smoke.checks.insert(
                        "search_native_save_run_reopen".into(),
                        serde_json::json!(
                            index
                                .entries
                                .iter()
                                .any(|e| Some(e.id.as_str()) == self.searches.saved_id())
                                && !snapshot.rows.is_empty()
                                && snapshot.revision == reopened.revision().unwrap()
                        ),
                    );
                    smoke.checks.insert(
                        "search_native_private".into(),
                        serde_json::json!(
                            snapshot.rows.iter().all(|r| r.player.is_empty())
                                && reopened.saved_searches(Role::Player).is_err()
                                && reopened
                                    .search_snapshot(&snapshot.query, false, Role::Player)
                                    .is_err()
                        ),
                    );
                    for statistics in [false, true] {
                        let html = grapevine_workbench::searches::html(
                            snapshot,
                            Language::Portuguese,
                            statistics,
                        )
                        .unwrap();
                        let name = if statistics {
                            "statistics.html"
                        } else {
                            "search.html"
                        };
                        let _ = std::fs::write(smoke.output_dir.join(name), html);
                    }
                    let path = smoke.output_dir.join("search-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::SearchDone;
                } else if let Some(message) = &self.searches.message {
                    smoke.failure = Some(format!("Search run failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::SearchDone => {
                if smoke.output_dir.join("search-smoke.bmp").is_file() {
                    self.searches.open = false;
                    smoke.phase = SmokePhase::TraitPrepare(false);
                }
            }
            SmokePhase::TraitPrepare(merits) => {
                if !merits {
                    let lib = self.library.as_mut().unwrap();
                    let id = lib
                        .summaries(Role::Storyteller)
                        .unwrap()
                        .into_iter()
                        .find(|r| {
                            r.build.is_none()
                                && grapevine_workbench::sheet::is_character_kind(&r.kind)
                        })
                        .unwrap()
                        .id;
                    let mut sheet = lib.sheet_edit(&id, Role::Storyteller).unwrap();
                    for (key, name, value) in [
                        ("InfluenceList", "Synthetic influence", "2"),
                        ("MeritList", "Synthetic merit", "2"),
                        ("FlawList", "Synthetic flaw", "descriptive"),
                    ] {
                        let mut list = grapevine_workbench::sheet::empty_list(key);
                        list["Traits"] = serde_json::json!([{"Name":name,"Value":value,"Note":"PRIVATE_TRAIT_NOTE"}]);
                        sheet.lists.insert(key.into(), list);
                    }
                    self.revision = Some(
                        lib.save_sheet(&id, &sheet, lib.revision().unwrap(), Role::Storyteller)
                            .unwrap(),
                    );
                }
                let kind = if merits {
                    grapevine_workbench::trait_statistics::Kind::MeritsFlaws
                } else {
                    grapevine_workbench::trait_statistics::Kind::Influence
                };
                self.searches = search_window::SearchWindow::synthetic_traits(kind);
                smoke.phase = SmokePhase::TraitRun(merits);
            }
            SmokePhase::TraitRun(merits) => {
                if let Some(rect) = probe(ctx, "search_run") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::TraitCheck(merits);
                }
            }
            SmokePhase::TraitCheck(merits) => {
                if let Some(report) = self.searches.prepared_traits() {
                    let html =
                        grapevine_workbench::trait_reports::html(report, Language::Portuguese)
                            .unwrap();
                    let key = if merits { "merits-flaws" } else { "influence" };
                    smoke.checks.insert(
                        format!("{key}_native_prepare_private"),
                        serde_json::json!(
                            report.report.buckets.len() == if merits { 2 } else { 1 }
                                && !html.contains("PRIVATE_TRAIT_NOTE")
                                && report.search.rows.iter().all(|r| !html.contains(&r.id))
                        ),
                    );
                    let _ = std::fs::write(smoke.output_dir.join(format!("{key}.html")), html);
                    let path = smoke.output_dir.join(format!("{key}-smoke.bmp"));
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::TraitDone(merits);
                } else if let Some(message) = &self.searches.message {
                    smoke.failure = Some(format!("Trait report failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::TraitDone(merits) => {
                let key = if merits { "merits-flaws" } else { "influence" };
                if smoke.output_dir.join(format!("{key}-smoke.bmp")).is_file() {
                    self.searches.open = false;
                    smoke.phase = if merits {
                        SmokePhase::DistributionPrepare(false)
                    } else {
                        SmokePhase::TraitPrepare(true)
                    };
                }
            }
            SmokePhase::DistributionPrepare(unknown) => {
                self.searches = search_window::SearchWindow::synthetic_distribution(unknown);
                smoke.phase = SmokePhase::DistributionRun(unknown);
            }
            SmokePhase::DistributionRun(unknown) => {
                if let Some(rect) = probe(ctx, "search_run") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::DistributionCheck(unknown);
                }
            }
            SmokePhase::DistributionCheck(unknown) => {
                if let Some(report) = self.searches.prepared_distribution() {
                    let html = grapevine_workbench::trait_distribution_report::html(
                        report,
                        Language::Portuguese,
                    )
                    .unwrap();
                    let key = if unknown {
                        "distribution-unknown"
                    } else {
                        "distribution-numeric"
                    };
                    smoke.checks.insert(
                        format!("{key}_native_private_readonly"),
                        serde_json::json!(
                            report.report.matched_characters == 1
                                && report.report.unknown_occurrences == usize::from(unknown)
                                && report.report.known_occurrences == usize::from(!unknown)
                                && !html.contains("PRIVATE_TRAIT_NOTE")
                                && report.search.rows.iter().all(|r| !html.contains(&r.id))
                                && report.search.revision
                                    == self.library.as_ref().unwrap().revision().unwrap()
                                && self
                                    .library
                                    .as_ref()
                                    .unwrap()
                                    .trait_distribution_snapshot(
                                        &report.search.query,
                                        false,
                                        &report.request,
                                        Role::Player
                                    )
                                    .is_err()
                        ),
                    );
                    std::fs::write(smoke.output_dir.join(format!("{key}.html")), html).unwrap();
                    let path = smoke.output_dir.join(format!("{key}-smoke.bmp"));
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::DistributionDone(unknown);
                } else if let Some(message) = &self.searches.message {
                    smoke.failure = Some(format!("Trait distribution failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::DistributionDone(unknown) => {
                let key = if unknown {
                    "distribution-unknown"
                } else {
                    "distribution-numeric"
                };
                if smoke.output_dir.join(format!("{key}-smoke.bmp")).is_file() {
                    self.searches.open = false;
                    smoke.phase = if unknown {
                        SmokePhase::BoonPrepare
                    } else {
                        SmokePhase::DistributionPrepare(true)
                    };
                }
            }
            SmokePhase::BoonPrepare => {
                let lib = self.library.as_ref().unwrap();
                let holder = self
                    .summaries
                    .iter()
                    .find(|r| r.kind == "Vampire" && r.build.is_none())
                    .unwrap();
                self.boons = boon_window::BoonWindow::synthetic(
                    lib.boon_snapshot(&holder.id, Role::Storyteller).unwrap(),
                );
                smoke.phase = SmokePhase::BoonSave;
            }
            SmokePhase::BoonSave => {
                if let Some(rect) = probe(ctx, "boon_save") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::BoonCheck;
                }
            }
            SmokePhase::BoonCheck => {
                if !self.boons.dirty()
                    && let Some(id) = self.boons.selected_id()
                {
                    let reopened = Library::open(&smoke.library_path).unwrap();
                    let holder = self.boons.holder_id().unwrap();
                    let snapshot = reopened.boon_snapshot(holder, Role::Storyteller).unwrap();
                    let entry = snapshot
                        .entries
                        .iter()
                        .find(|entry| entry.id == id)
                        .unwrap();
                    smoke.checks.insert(
                        "boon_native_save_reopen".into(),
                        serde_json::json!(
                            entry.edit.partner == "Synthetic partner"
                                && entry.edit.owed_by_holder == Some(true)
                                && entry.edit.day == "2026-01-02"
                                && entry.edit.description == "PRIVATE_BOON_DESCRIPTION"
                        ),
                    );
                    let public =
                        serde_json::to_string(&reopened.record(holder, Role::Player).unwrap())
                            .unwrap();
                    smoke.checks.insert(
                        "boon_native_private".into(),
                        serde_json::json!(
                            !public.contains("PRIVATE_BOON_DESCRIPTION")
                                && reopened.boon_snapshot(holder, Role::Player).is_err()
                        ),
                    );
                    let path = smoke.output_dir.join("boon-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::BoonDone;
                } else if let Some(message) = &self.boons.message {
                    smoke.failure = Some(format!("Boon save failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::BoonDone => {
                if smoke.output_dir.join("boon-smoke.bmp").is_file() {
                    self.boons.open = false;
                    smoke.phase = SmokePhase::StatusPrepare;
                }
            }
            SmokePhase::StatusPrepare => {
                self.operations = operations_window::OperationsWindow::synthetic_status();
                smoke.phase = SmokePhase::StatusClick;
            }
            SmokePhase::StatusClick => {
                if let Some(rect) = probe(ctx, "operations_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::StatusCheck;
                }
            }
            SmokePhase::StatusCheck => {
                if self.operations.is_prepared() {
                    let lib = self.library.as_ref().unwrap();
                    let snapshot = lib
                        .vampire_status_snapshot("", true, false, Role::Storyteller)
                        .unwrap();
                    let html =
                        grapevine_workbench::vampire_status::html(&snapshot, Language::Portuguese)
                            .unwrap();
                    smoke.checks.insert(
                        "status_native_prepare_private".into(),
                        serde_json::json!(
                            snapshot
                                .characters
                                .iter()
                                .any(|c| c.boons.iter().any(|b| b.partner == "Synthetic partner"))
                                && !html.contains("PRIVATE_BOON_DESCRIPTION")
                                && snapshot.search.rows.iter().all(|r| !html.contains(&r.id))
                                && probe(ctx, "operations_export").is_some()
                        ),
                    );
                    let _ = std::fs::write(smoke.output_dir.join("vampire-status.html"), html);
                    let path = smoke.output_dir.join("vampire-status-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::StatusDone;
                } else if let Some(message) = &self.operations.message {
                    smoke.failure = Some(format!("Status prepare failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::StatusDone => {
                if smoke.output_dir.join("vampire-status-smoke.bmp").is_file() {
                    self.operations.open = false;
                    smoke.phase = SmokePhase::ActionLinkPrepare;
                }
            }
            SmokePhase::ActionLinkPrepare => {
                let character = self
                    .summaries
                    .iter()
                    .find(|row| row.kind == "Vampire" && row.build.is_none())
                    .unwrap()
                    .clone();
                let lib = self.library.as_mut().unwrap();
                let source = grapevine_workbench::actions::Action {
                    character: character.name.clone(),
                    day: "2026-11-01".into(),
                    slots: vec![grapevine_workbench::actions::ActivitySlot {
                        name: "Synthetic previous activity".into(),
                        level: "2".into(),
                        unused: "3".into(),
                        total: "6".into(),
                        growth: "1".into(),
                        action: "SOURCE_ACTION_TEXT".into(),
                        result: "SOURCE_RESULT_TEXT".into(),
                    }],
                };
                let id = lib
                    .create_linked_action(
                        &source,
                        Some(&character.id),
                        lib.revision().unwrap(),
                        Role::Storyteller,
                    )
                    .unwrap();
                self.revision = Some(lib.revision().unwrap());
                self.story =
                    story_window::StoryWindow::synthetic_linked(character, id, self.revision);
                smoke.phase = SmokePhase::ActionTimelineClick;
            }
            SmokePhase::ActionTimelineClick => {
                if let Some(rect) = probe(ctx, "action_timeline_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ActionTimelineCheck;
                }
            }
            SmokePhase::ActionTimelineCheck => {
                if self.story.timeline.is_some() {
                    smoke.phase = SmokePhase::ActionCopyClick;
                } else if let Some(message) = &self.story.message {
                    smoke.failure = Some(format!("Action timeline failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::ActionCopyClick => {
                if let Some(rect) = probe(ctx, "action_copy_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ActionCopyCheck;
                }
            }
            SmokePhase::ActionCopyCheck => {
                if let Some(preview) = &self.story.copy_preview {
                    smoke.checks.insert("action_copy_native_preview".into(),serde_json::json!(
                        preview.draft.slots[0].unused=="3" && preview.draft.slots[0].growth=="1"
                            && preview.draft.slots[0].action.is_empty() && preview.draft.slots[0].result.is_empty()
                            && self.library.as_ref().unwrap().revision().unwrap()==preview.revision
                            && matches!(&self.story.draft,grapevine_workbench::story_work::Draft::Action(action) if action.slots.is_empty())
                    ));
                    smoke.phase = SmokePhase::ActionCopyApply;
                } else if let Some(message) = &self.story.message {
                    smoke.failure = Some(format!("Action preview failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::ActionCopyApply => {
                if let Some(rect) = probe(ctx, "action_copy_apply") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ActionCopyApplied;
                }
            }
            SmokePhase::ActionCopyApplied => {
                if self.story.copy_preview.is_none()
                    && matches!(&self.story.draft,grapevine_workbench::story_work::Draft::Action(action) if action.slots.len()==1)
                {
                    smoke.phase = SmokePhase::ActionCopySave;
                }
            }
            SmokePhase::ActionCopySave => {
                if let Some(rect) = probe(ctx, "story_save") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ActionCopySaved;
                }
            }
            SmokePhase::ActionCopySaved => {
                if !self.story.dirty()
                    && let Some(id) = &self.story.target
                {
                    let reopened = Library::open(&smoke.library_path).unwrap();
                    let saved = reopened
                        .linked_action_snapshot(id, Role::Storyteller)
                        .unwrap();
                    let character = &saved.binding.as_ref().unwrap().binding.character_id;
                    let timeline = reopened
                        .action_timeline(character, "2026-11-01", "2026-11-30", Role::Storyteller)
                        .unwrap();
                    let html = grapevine_workbench::action_timeline_report::html(
                        &timeline,
                        Language::Portuguese,
                    )
                    .unwrap();
                    smoke.checks.insert("action_copy_native_save_reopen".into(),serde_json::json!(
                        matches!(&saved.story.draft,grapevine_workbench::story_work::Draft::Action(action) if action.slots[0].unused=="3" && action.slots[0].growth=="1" && action.slots[0].action.is_empty() && action.slots[0].result.is_empty())
                            && timeline.rows.len()==2
                    ));
                    smoke.checks.insert(
                        "action_timeline_native_private".into(),
                        serde_json::json!(
                            !html.contains(character)
                                && timeline
                                    .rows
                                    .iter()
                                    .all(|row| !html.contains(&row.action_id))
                                && reopened.linked_action_snapshot(id, Role::Player).is_err()
                                && reopened
                                    .action_timeline(
                                        character,
                                        "2026-11-01",
                                        "2026-11-30",
                                        Role::Player
                                    )
                                    .is_err()
                        ),
                    );
                    let _ = std::fs::write(smoke.output_dir.join("action-timeline.html"), html);
                    self.story.timeline = Some(timeline);
                    let path = smoke.output_dir.join("linked-action-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::ActionLinkDone;
                } else if let Some(message) = &self.story.message {
                    smoke.failure = Some(format!("Linked Action save failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::ActionLinkDone => {
                if smoke.output_dir.join("linked-action-smoke.bmp").is_file() {
                    smoke.phase = SmokePhase::CombinedToggle;
                }
            }
            SmokePhase::CombinedToggle => {
                if let Some(rect) = probe(ctx, "combined_story_option") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::CombinedToggleCheck;
                }
            }
            SmokePhase::CombinedToggleCheck => {
                if self.story.include_rumors
                    && self.story.timeline.is_none()
                    && self.story.combined.is_none()
                {
                    smoke.phase = SmokePhase::CombinedPrepare;
                }
            }
            SmokePhase::CombinedPrepare => {
                if let Some(rect) = probe(ctx, "action_timeline_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::CombinedCheck;
                }
            }
            SmokePhase::CombinedCheck => {
                if let Some(snapshot) = &self.story.combined {
                    let html = grapevine_workbench::combined_story_report::html(
                        snapshot,
                        Language::Portuguese,
                    )
                    .unwrap();
                    let lib = self.library.as_ref().unwrap();
                    smoke.checks.insert(
                        "combined_native_readonly_private".into(),
                        serde_json::json!(
                            snapshot.actions.revision == lib.revision().unwrap()
                                && !html.contains("SMOKE_ONLY_SECRET")
                                && !html.contains(&snapshot.request.character_id)
                                && lib
                                    .combined_story_snapshot(&snapshot.request, Role::Player)
                                    .is_err()
                        ),
                    );
                    let rumor = html.find("The key bears");
                    let earlier = html.find("SOURCE_ACTION_TEXT");
                    let later = html.rfind("Synthetic previous activity");
                    smoke.checks.insert(
                        "combined_native_exact_audience_and_order".into(),
                        serde_json::json!(
                            snapshot.actions.rows.len() == 2
                                && snapshot.rumors.len() == 1
                                && rumor.is_some()
                                && earlier.is_some()
                                && later.is_some()
                                && earlier < rumor
                                && later > rumor
                        ),
                    );
                    std::fs::write(smoke.output_dir.join("combined-story.html"), html).unwrap();
                    let path = smoke.output_dir.join("combined-story-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::CombinedDone;
                } else if let Some(message) = &self.story.message {
                    smoke.failure = Some(format!("Combined story failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::CombinedDone => {
                if smoke.output_dir.join("combined-story-smoke.bmp").is_file() {
                    self.story.open = false;
                    smoke.phase = SmokePhase::ChroniclePrepare;
                }
            }
            SmokePhase::ChroniclePrepare => {
                self.chronicle = chronicle_window::ChronicleWindow::synthetic(
                    self.library
                        .as_ref()
                        .unwrap()
                        .chronicle_profile(Role::Storyteller)
                        .unwrap(),
                );
                self.chronicle.edit.title = "Synthetic Chronicle".into();
                self.chronicle.edit.description = "SYNTHETIC_GAME_DETAILS".into();
                self.chronicle.edit.house_rules = "Staff-entered synthetic rules".into();
                self.chronicle.edit.email = "PRIVATE_CHRONICLE_CONTACT".into();
                self.chronicle.edit.private_notes = "PRIVATE_CHRONICLE_NOTES".into();
                self.chronicle.edit.publish_details = true;
                smoke.phase = SmokePhase::ChronicleSave;
            }
            SmokePhase::ChronicleSave => {
                if let Some(rect) = ctx
                    .data(|d| d.get_temp::<Option<egui::Rect>>(egui::Id::unique("chronicle_save")))
                    .flatten()
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ChronicleSaved;
                }
            }
            SmokePhase::ChronicleSaved => {
                if !self.chronicle.dirty() {
                    let reopened = Library::open(&self.library.as_ref().unwrap().path).unwrap();
                    let private = reopened.chronicle_profile(Role::Storyteller).unwrap();
                    let public =
                        serde_json::to_string(&reopened.public_chronicle_profile().unwrap())
                            .unwrap();
                    smoke.checks.insert(
                        "chronicle_native_save_public_scope".into(),
                        serde_json::json!(
                            private.profile.title == "Synthetic Chronicle"
                                && private.profile.email == "PRIVATE_CHRONICLE_CONTACT"
                                && public.contains("SYNTHETIC_GAME_DETAILS")
                                && !public.contains("PRIVATE_CHRONICLE_CONTACT")
                                && !public.contains("PRIVATE_CHRONICLE_NOTES")
                                && reopened.chronicle_profile(Role::Player).is_err()
                        ),
                    );
                    self.chronicle.handover_tab = true;
                    smoke.phase = SmokePhase::ChronicleHandover;
                } else if let Some(message) = &self.chronicle.message {
                    smoke.failure = Some(format!("Chronicle save failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::ChronicleHandover => {
                if let Some(rect) = ctx
                    .data(|d| {
                        d.get_temp::<Option<egui::Rect>>(egui::Id::unique("chronicle_prepare"))
                    })
                    .flatten()
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ChronicleChecked;
                }
            }
            SmokePhase::ChronicleChecked => {
                if let Some(h) = &self.chronicle.handover {
                    let html =
                        grapevine_workbench::chronicle_handover::html(h, Language::Portuguese)
                            .unwrap();
                    std::fs::write(smoke.output_dir.join("chronicle-handover.html"), &html)
                        .unwrap();
                    smoke.checks.insert(
                        "chronicle_native_handover_private_readonly".into(),
                        serde_json::json!(
                            !html.contains("PRIVATE_CHRONICLE_CONTACT")
                                && !html.contains("PRIVATE_CHRONICLE_NOTES")
                                && h.revision == self.library.as_ref().unwrap().revision().unwrap()
                                && self
                                    .library
                                    .as_ref()
                                    .unwrap()
                                    .chronicle_handover(false, false, false, Role::Player)
                                    .is_err()
                        ),
                    );
                    let path = smoke.output_dir.join("chronicle-handover-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::ChronicleDone;
                } else if let Some(message) = &self.chronicle.message {
                    smoke.failure = Some(format!("Chronicle handover failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::ChronicleDone => {
                if smoke
                    .output_dir
                    .join("chronicle-handover-smoke.bmp")
                    .is_file()
                {
                    self.chronicle.open = false;
                    smoke.phase = SmokePhase::SupportPrepare;
                }
            }
            SmokePhase::SupportPrepare => {
                use std::io::Write;
                let source = smoke.output_dir.join("support-source");
                diagnostics::write(&source, "synthetic_support", "EXPECTED_ERROR", true).unwrap();
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(source.join("events.jsonl"))
                    .unwrap();
                writeln!(file,"{{\"unix_seconds\":2,\"build\":\"0.29.0\",\"operation\":\"synthetic\",\"code\":\"EXPECTED\",\"backtrace\":\"PRIVATE_TRACE_SECRET\",\"unknown\":\"PRIVATE_SUPPORT_SECRET\"}}").unwrap();
                writeln!(file, "malformed PRIVATE_SUPPORT_SECRET").unwrap();
                self.support = support_window::SupportWindow::synthetic(source);
                smoke.phase = SmokePhase::SupportPreview;
            }
            SmokePhase::SupportPreview => {
                if let Some(rect) = probe(ctx, "support_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::SupportDefaultCheck;
                }
            }
            SmokePhase::SupportDefaultCheck => {
                if let Some(snapshot) = &self.support.snapshot {
                    let json = grapevine_workbench::support_export::json(snapshot).unwrap();
                    self.support
                        .save_new(
                            &smoke.output_dir.join("support-default.json"),
                            Role::Storyteller,
                        )
                        .unwrap();
                    smoke.checks.insert(
                        "support_native_default_private".into(),
                        serde_json::json!(
                            snapshot.events.len() == 2
                                && snapshot.malformed_lines == 1
                                && snapshot.events.iter().all(|e| e.backtrace.is_none())
                                && !json.contains("PRIVATE_TRACE_SECRET")
                                && !json.contains("PRIVATE_SUPPORT_SECRET")
                        ),
                    );
                    smoke.phase = SmokePhase::SupportToggleTraces;
                } else if let Some(message) = &self.support.message {
                    smoke.failure = Some(format!("Support preview failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::SupportToggleTraces => {
                if let Some(rect) = probe(ctx, "support_traces") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::SupportToggleCheck;
                }
            }
            SmokePhase::SupportToggleCheck => {
                if self.support.include_backtraces && self.support.snapshot.is_none() {
                    smoke.phase = SmokePhase::SupportTracePreview;
                }
            }
            SmokePhase::SupportTracePreview => {
                if let Some(rect) = probe(ctx, "support_prepare") {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::SupportTraceCheck;
                }
            }
            SmokePhase::SupportTraceCheck => {
                if let Some(snapshot) = &self.support.snapshot {
                    let json = grapevine_workbench::support_export::json(snapshot).unwrap();
                    self.support
                        .save_new(
                            &smoke.output_dir.join("support-trace.json"),
                            Role::Storyteller,
                        )
                        .unwrap();
                    smoke.checks.insert(
                        "support_native_trace_optin_save".into(),
                        serde_json::json!(
                            snapshot.include_backtraces
                                && snapshot.events.len() == 2
                                && json.contains("PRIVATE_TRACE_SECRET")
                                && !json.contains("PRIVATE_SUPPORT_SECRET")
                                && self
                                    .support
                                    .save_new(
                                        &smoke.output_dir.join("support-denied.json"),
                                        Role::Player
                                    )
                                    .is_err()
                        ),
                    );
                    let path = smoke.output_dir.join("support-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&path, &image);
                    });
                    smoke.phase = SmokePhase::SupportDone;
                } else if let Some(message) = &self.support.message {
                    smoke.failure = Some(format!("Support trace preview failed: {message}"));
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::SupportDone => {
                if smoke.output_dir.join("support-smoke.bmp").is_file() {
                    self.support.open = false;
                    smoke.phase = SmokePhase::BuilderAfterXp;
                }
            }
            SmokePhase::BuilderAfterXp => {
                if let Some(rect) = self.smoke_controls.builder {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::ClickBuildTest;
                }
            }
            SmokePhase::ClickBuildTest => {
                if self.route == WorkArea::Builder {
                    smoke
                        .checks
                        .insert("builder_route_clicked".into(), serde_json::json!(true));
                    if let Some(rect) = self.smoke_controls.create_build {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::CreateDraft;
                    }
                } else {
                    smoke.failure = Some("Builder route did not open".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::CreateDraft => {
                if self.selected_record.as_ref().and_then(|r| r.build) == Some(BuildKind::Draft) {
                    smoke
                        .checks
                        .insert("draft_copy_created".into(), serde_json::json!(true));
                    smoke.phase = SmokePhase::ScrollTrait;
                } else {
                    smoke.failure =
                        Some("Draft creation button did not create separate copy".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::ScrollTrait => {
                let rect = ctx
                    .data(|d| {
                        d.get_temp::<Option<egui::Rect>>(egui::Id::unique("sheet_value_probe"))
                    })
                    .flatten();
                if let Some(rect) = rect {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::TypeTrait;
                } else {
                    scroll_detail(&mut self.pending_raw_events, ctx, -220.0);
                }
            }
            SmokePhase::TypeTrait => {
                if let Some(rect) = ctx
                    .data(|d| {
                        d.get_temp::<Option<egui::Rect>>(egui::Id::unique("sheet_value_probe"))
                    })
                    .flatten()
                {
                    inject_replace_text(
                        &mut self.pending_raw_events,
                        rect,
                        "Smoke Descriptive Trait",
                    );
                    smoke.phase = SmokePhase::CheckTrait;
                }
            }
            SmokePhase::CheckTrait => {
                if self
                    .sheet_edit
                    .as_ref()
                    .and_then(|s| s.lists.get("DisciplineList"))
                    .and_then(|v| v.pointer("/Traits/0/Value"))
                    .and_then(serde_json::Value::as_str)
                    == Some("Smoke Descriptive Trait")
                {
                    smoke
                        .checks
                        .insert("trait_text_input".into(), serde_json::json!(true));
                    if let Some(rect) = self.smoke_controls.english {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::ScrollSaveDraft;
                    }
                } else {
                    smoke.failure =
                        Some("Native trait text input did not update the sheet draft".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::ScrollSaveDraft => {
                let kept = self
                    .sheet_edit
                    .as_ref()
                    .and_then(|s| s.lists.get("DisciplineList"))
                    .and_then(|v| v.pointer("/Traits/0/Value"))
                    .and_then(serde_json::Value::as_str)
                    == Some("Smoke Descriptive Trait");
                if self.language != Language::English || !kept {
                    smoke.failure = Some("Sheet draft lost across language change".into());
                    smoke.phase = SmokePhase::Capture;
                } else if let Some(rect) = self.smoke_controls.save {
                    let screenshot = smoke.output_dir.join("editor-smoke.bmp");
                    ctx.request_screenshot(move |image| {
                        let _ = write_bmp(&screenshot, &image);
                    });
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::SaveDraft;
                    smoke.checks.insert(
                        "sheet_draft_preserved_on_language_change".into(),
                        serde_json::json!(true),
                    );
                } else {
                    scroll_detail(&mut self.pending_raw_events, ctx, 220.0);
                }
            }
            SmokePhase::SaveDraft => {
                if unsaved_edit {
                    smoke.failure = Some("Draft sheet save failed".into());
                    smoke.phase = SmokePhase::Capture;
                } else if let Some(rect) = self.smoke_controls.build_test {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::CreateTest;
                    smoke
                        .checks
                        .insert("draft_sheet_saved".into(), serde_json::json!(true));
                }
            }
            SmokePhase::CreateTest => {
                if self.route == WorkArea::BuildTest {
                    if let Some(rect) = self.smoke_controls.create_build {
                        inject_click(&mut self.pending_raw_events, rect);
                        smoke.phase = SmokePhase::CheckTest;
                    }
                } else {
                    smoke.failure = Some("Test route did not open".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::CheckTest => {
                if self.selected_record.as_ref().and_then(|r| r.build) == Some(BuildKind::Test) {
                    smoke
                        .checks
                        .insert("test_copy_created".into(), serde_json::json!(true));
                    smoke.phase = SmokePhase::ClickPlayer;
                } else {
                    smoke.failure =
                        Some("Test creation button did not create separate copy".into());
                    smoke.phase = SmokePhase::Capture;
                }
            }
            SmokePhase::ClickPlayer => {
                if let Some(rect) = self.smoke_controls.player {
                    inject_click(&mut self.pending_raw_events, rect);
                    smoke.phase = SmokePhase::Capture;
                    smoke
                        .checks
                        .insert("build_test_route_clicked".into(), serde_json::json!(true));
                }
            }
            SmokePhase::Capture => {
                smoke.checks.insert(
                    "player_points_private_state_cleared".into(),
                    serde_json::json!(
                        !self.points_open
                            && self.points_target.is_none()
                            && self.points_ledger.is_none()
                            && self.points_form.selected_player.is_none()
                            && !self.points_form.dirty
                    ),
                );
                smoke.checks.insert(
                    "player_session_private_state_cleared".into(),
                    serde_json::json!(
                        !self.session_open
                            && self.session_form.receipts.is_empty()
                            && self.session_form.quote.is_none()
                            && !self.session_form.dirty
                    ),
                );
                smoke.checks.insert(
                    "player_print_private_state_cleared".into(),
                    serde_json::json!(
                        self.route == WorkArea::Player
                            && self.print_target.is_none()
                            && self.print_form.document.is_none()
                            && !self.print_open
                    ),
                );
                smoke.checks.insert(
                    "player_playtime_private_state_cleared".into(),
                    serde_json::json!(
                        self.route == WorkArea::Player
                            && self.playtime_target.is_none()
                            && self.playtime_form.report.is_none()
                            && !self.playtime_open
                    ),
                );
                if smoke.failure.is_none()
                    && self.route != WorkArea::Player
                    && let Some(rect) = self.smoke_controls.player
                {
                    inject_click(&mut self.pending_raw_events, rect);
                    ctx.request_repaint_after(Duration::from_millis(40));
                    return;
                }
                if let Some(smoke) = self.smoke.as_mut() {
                    let screenshot_path = smoke.output_dir.join("ui-smoke.bmp");
                    let screenshot_state = Arc::clone(&smoke.screenshot_state);
                    ctx.request_screenshot(move |image| {
                        let result =
                            write_bmp(&screenshot_path, &image).map_err(|error| error.to_string());
                        if let Ok(mut state) = screenshot_state.lock() {
                            *state = Some(result);
                        }
                    });
                    smoke.phase = SmokePhase::AwaitCapture;
                    smoke.checks.insert(
                        "player_xp_private_state_cleared".into(),
                        serde_json::json!(
                            self.route == WorkArea::Player
                                && !self.xp_window_open
                                && self.xp_view.is_none()
                                && self.xp_roster.is_none()
                                && self.xp_form == crate::xp_ui::XpForm::default()
                        ),
                    );
                    smoke.checks.insert(
                        "player_route_final".into(),
                        serde_json::json!(self.route == WorkArea::Player),
                    );
                }
            }
            SmokePhase::AwaitCapture => {
                let capture = smoke
                    .screenshot_state
                    .lock()
                    .ok()
                    .and_then(|mut state| state.take());
                if let Some(result) = capture {
                    if let Err(error) = result {
                        smoke.failure = Some(format!("Screenshot capture failed: {error}"));
                    }
                    self.finish_smoke(ctx);
                } else {
                    ctx.request_repaint_after(Duration::from_millis(40));
                }
            }
        }
    }

    fn finish_smoke(&mut self, ctx: &egui::Context) {
        let Some(smoke) = self.smoke.as_mut() else {
            return;
        };
        let mut story_view_ok = false;
        let mut player_view_redacted = false;
        let mut reopened = false;
        if let Some(id) = self.selected_id.as_ref()
            && let Ok(library) = Library::open(&smoke.library_path)
            && let (Ok(story_view), Ok(player_view)) = (
                library.record(id, Role::Storyteller),
                library.record(id, Role::Player),
            )
        {
            smoke.checks.insert(
                "native_identity_player_redacted_after_reopen".into(),
                serde_json::json!(
                    story_view.fields["Clan"] == "Native Clan [ST]NATIVE_PRIVATE_CLAN[/ST]"
                        && player_view.fields["Clan"] == "Native Clan "
                ),
            );
            story_view_ok = story_view.name == "Smoke Edited Character"
                && story_view.status == "Draft"
                && story_view.notes == "SMOKE_ONLY_SECRET";
            player_view_redacted = player_view.name == "Smoke Edited Character"
                && player_view.notes.is_empty()
                && player_view.status == "Draft";
            reopened = true;
        }
        smoke
            .checks
            .insert("saved_record_reopened".into(), serde_json::json!(reopened));
        smoke.checks.insert(
            "storyteller_projection_checked".into(),
            serde_json::json!(story_view_ok),
        );
        smoke.checks.insert(
            "player_projection_checked".into(),
            serde_json::json!(player_view_redacted),
        );
        if !reopened || !story_view_ok || !player_view_redacted {
            smoke.failure.get_or_insert_with(|| {
                "Reopened library did not match expected saved projections".to_owned()
            });
        }
        let build_isolation = Library::open(&smoke.library_path)
            .ok()
            .is_some_and(|library| {
                let Ok(mut rows) = library.summaries(Role::Storyteller) else {
                    return false;
                };
                rows.retain(|row| grapevine_workbench::sheet::is_character_kind(&row.kind));
                if rows.len() != 3
                    || library.summaries(Role::Player).map_or(true, |r| {
                        r.iter()
                            .filter(|row| grapevine_workbench::sheet::is_character_kind(&row.kind))
                            .count()
                            != 1
                    })
                {
                    return false;
                }
                rows.iter().all(|row| {
                    library
                        .sheet_edit(&row.id, Role::Storyteller)
                        .ok()
                        .and_then(|s| s.lists.get("DisciplineList").cloned())
                        .and_then(|v| {
                            v.pointer("/Traits/0/Value")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned)
                        })
                        .is_some_and(|v| {
                            v == if row.build == Some(BuildKind::Draft) {
                                "Smoke Descriptive Trait"
                            } else {
                                "Original Level"
                            }
                        })
                })
            });
        smoke.checks.insert(
            "reopened_builds_isolated_from_source".into(),
            serde_json::json!(build_isolation),
        );
        smoke.checks.insert(
            "editor_screenshot_retained".into(),
            serde_json::json!(smoke.output_dir.join("editor-smoke.bmp").is_file()),
        );
        if !build_isolation {
            smoke
                .failure
                .get_or_insert("Build/source isolation reopen check failed".into());
        }
        let xp_ok = Library::open(&smoke.library_path)
            .ok()
            .and_then(|l| {
                self.selected_id
                    .as_ref()
                    .and_then(|id| l.xp_view(id, Role::Storyteller).ok())
            })
            .is_some_and(|v| {
                v.unspent.as_deref() == Some("12.25")
                    && v.ledger.is_some_and(|l| l.entries.len() == 4)
            });
        smoke.checks.insert(
            "xp_reopened_balance_and_entry_count".into(),
            serde_json::json!(xp_ok),
        );
        let purchase_ok = Library::open(&smoke.library_path)
            .ok()
            .and_then(|lib| {
                self.summaries
                    .iter()
                    .find(|s| s.build.is_none() && s.kind == "Vampire")
                    .and_then(|s| lib.purchase_receipts(&s.id, Role::Storyteller).ok())
            })
            .is_some_and(|r| r.len() == 1 && r[0].quote.price.price.total == "4");
        smoke.checks.insert(
            "purchase_receipt_reopened".into(),
            serde_json::json!(purchase_ok),
        );
        smoke.checks.insert(
            "purchase_screenshot_retained".into(),
            serde_json::json!(
                smoke
                    .output_dir
                    .join("purchase-receipt-smoke.bmp")
                    .is_file()
            ),
        );
        smoke.checks.insert(
            "player_purchase_private_state_cleared".into(),
            serde_json::json!(
                !self.purchase_open
                    && self.purchase_target.is_none()
                    && self.purchase_form.receipts.is_empty()
                    && self.purchase_form.quote.is_none()
                    && self.purchase_form.cancel_purchase.is_none()
                    && self.purchase_form.cancel_quote.is_none()
                    && self.purchase_form.plan.is_empty()
                    && self.purchase_form.plan_quote.is_none()
                    && !self.purchase_form.dirty
            ),
        );
        let cancel_ok = Library::open(&smoke.library_path)
            .ok()
            .and_then(|l| {
                self.selected_id
                    .as_ref()
                    .and_then(|id| l.purchase_receipts(id, Role::Storyteller).ok())
            })
            .is_some_and(|r| {
                r.len() == 1
                    && r[0]
                        .cancellation
                        .as_ref()
                        .is_some_and(|c| c.quote.refund == "4")
            });
        smoke.checks.insert(
            "cancellation_receipt_reopened".into(),
            serde_json::json!(cancel_ok),
        );
        smoke.checks.insert(
            "creation_screenshot_retained".into(),
            serde_json::json!(
                smoke
                    .output_dir
                    .join("creation-preview-smoke.bmp")
                    .is_file()
            ),
        );
        smoke.checks.insert(
            "cancellation_screenshot_retained".into(),
            serde_json::json!(
                smoke
                    .output_dir
                    .join("cancellation-receipt-smoke.bmp")
                    .is_file()
            ),
        );
        let reopened_menu = Library::open(&smoke.library_path)
            .ok()
            .and_then(|l| l.active_menu(Role::Storyteller).ok().flatten());
        smoke.checks.insert(
            "menu_selection_reopened".into(),
            serde_json::json!(
                reopened_menu
                    .is_some_and(|m| m.rows.len() == 3 && m.rows[2].name == "Guild Fixture")
            ),
        );
        smoke.checks.insert(
            "menu_screenshot_retained".into(),
            serde_json::json!(smoke.output_dir.join("menu-preview-smoke.bmp").is_file()),
        );
        smoke.checks.insert(
            "scenario_screenshot_retained".into(),
            serde_json::json!(
                smoke
                    .output_dir
                    .join("spending-preview-smoke.bmp")
                    .is_file()
            ),
        );
        smoke.checks.insert(
            "player_scenario_private_state_cleared".into(),
            serde_json::json!(
                !self.scenario_open
                    && self.scenario_baseline.is_none()
                    && self.scenario_balance.is_none()
                    && self.scenario_form.allocation == crate::scenario_ui::Allocation::default()
                    && self.scenario_form.comparisons.iter().all(Option::is_none)
            ),
        );
        smoke.checks.insert(
            "player_menu_private_state_cleared".into(),
            serde_json::json!(self.menu_preview.is_none() && self.menu_query.is_empty()),
        );
        smoke.checks.insert(
            "xp_audit_screenshot_retained".into(),
            serde_json::json!(smoke.output_dir.join("xp-audit-smoke.bmp").is_file()),
        );
        smoke.checks.insert(
            "xp_audit_summary_retained".into(),
            serde_json::json!(smoke.output_dir.join("xp-audit-summary.json").is_file()),
        );
        if !xp_ok {
            smoke
                .failure
                .get_or_insert("XP ledger reopen check failed".into());
        }
        smoke.checks.insert(
            "screenshot_requested_before_close".into(),
            serde_json::json!(true),
        );
        let screenshot_path = smoke.output_dir.join("ui-smoke.bmp");
        let result_path = smoke.output_dir.join("ui-smoke.json");
        let value = serde_json::json!({
            "ok": smoke.failure.is_none(),
            "mode": "native_ui_smoke_with_egui_pointer_and_text_events",
            "library_path": smoke.library_path,
            "screenshot_path": screenshot_path,
            "frames": smoke.frames,
            "checks": smoke.checks,
            "failure": smoke.failure,
            "limitations": "Native egui control events and rendering were exercised; operating-system file dialogs and human mouse/keyboard use were not tested. Synthetic opening/evidence and posting kinds were prefilled; buttons and amount fields exercised native inputs. Exact manual XP posting/rejection and build isolation were exercised. Two-power source-bound local quote/commit was exercised with synthetic declarations. Other rule costs and verified organizational approval remain unimplemented."
        });
        if let Err(error) = std::fs::write(
            &result_path,
            serde_json::to_vec_pretty(&value).unwrap_or_default(),
        ) {
            diagnostics::record("ui_smoke.write_result", "FILE_ERROR");
            eprintln!("Could not write UI smoke result: {error}");
        }
        self.smoke = None;
        self.close_after_confirmation = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

fn smoke_batch_request(second: bool) -> grapevine_workbench::purchases::PurchaseRequest {
    use grapevine_workbench::{
        pricing::{ClanRelation, PricingProfile},
        purchase_preview::{ANIMALISM_BECKONING_ID, ANIMALISM_FERAL_WHISPERS_ID},
        purchases::{PurchaseRequest, TeacherRule},
    };
    PurchaseRequest {
        review_id: None,
        id: uuid::Uuid::new_v4().to_string(),
        power_id: if second {
            ANIMALISM_BECKONING_ID
        } else {
            ANIMALISM_FERAL_WHISPERS_ID
        }
        .into(),
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        relation: Some(if second {
            ClanRelation::OutOfClan
        } else {
            ClanRelation::InClan
        }),
        affiliation_name: "Synthetic clan".into(),
        day: "2026-01-02".into(),
        available_from: "2026-01-03".into(),
        next_session_reference: "Synthetic next session".into(),
        actor: "Synthetic ST".into(),
        reason: "Synthetic advancement".into(),
        policy_version: "Synthetic v1".into(),
        policy_reference: "Synthetic local policy".into(),
        affiliation_evidence: "Synthetic affiliation".into(),
        approval_reference: "Synthetic approval".into(),
        eligibility_evidence: "Synthetic eligibility".into(),
        teacher_rule: Some(TeacherRule::Required),
        teacher_evidence: "Synthetic qualified teacher".into(),
        standard_cost_confirmed: true,
        owbn_evidence: Default::default(),
    }
}

fn probe(ctx: &egui::Context, key: &str) -> Option<egui::Rect> {
    ctx.data(|d| d.get_temp::<Option<egui::Rect>>(egui::Id::unique(key)))
        .flatten()
}
fn scroll_at(events: &mut Vec<egui::Event>, position: egui::Pos2, delta: f32) {
    events.push(egui::Event::PointerMoved(position));
    events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        delta: egui::vec2(0.0, delta),
        modifiers: egui::Modifiers::NONE,
    });
}
fn scroll_xp(events: &mut Vec<egui::Event>, ctx: &egui::Context, delta: f32) {
    events.push(egui::Event::PointerMoved(ctx.content_rect().center()));
    events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        delta: egui::vec2(0.0, delta),
        modifiers: egui::Modifiers::NONE,
    });
}

fn scroll_detail(events: &mut Vec<egui::Event>, ctx: &egui::Context, delta: f32) {
    let pos = probe(ctx, "record_detail_scroll")
        .map(|rect| rect.center())
        .unwrap_or_else(|| ctx.content_rect().center() + egui::vec2(180.0, 40.0));
    events.push(egui::Event::PointerMoved(pos));
    events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        delta: egui::vec2(0.0, delta),
        modifiers: egui::Modifiers::NONE,
    });
}

fn inject_click(events: &mut Vec<egui::Event>, rect: egui::Rect) {
    let pos = rect.center();
    {
        events.push(egui::Event::PointerMoved(pos));
        events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
}

fn inject_replace_text(events: &mut Vec<egui::Event>, _rect: egui::Rect, text: &str) {
    events.push(egui::Event::Key {
        key: egui::Key::A,
        physical_key: Some(egui::Key::A),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        },
    });
    events.push(egui::Event::Text(text.to_owned()));
    events.push(egui::Event::Key {
        key: egui::Key::A,
        physical_key: Some(egui::Key::A),
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
}

fn write_bmp(path: &std::path::Path, image: &egui::ColorImage) -> std::io::Result<()> {
    use std::io::Write;

    let width = image.size[0];
    let height = image.size[1];
    if width == 0 || height == 0 || width > i32::MAX as usize || height > i32::MAX as usize {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "screenshot dimensions are invalid",
        ));
    }
    let row_bytes = (width * 3 + 3) & !3;
    let pixel_bytes = row_bytes.checked_mul(height).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "screenshot is too large")
    })?;
    let total_bytes = 54usize.checked_add(pixel_bytes).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "screenshot is too large")
    })?;
    let mut file = std::fs::File::create(path)?;
    file.write_all(b"BM")?;
    file.write_all(&(total_bytes as u32).to_le_bytes())?;
    file.write_all(&[0; 4])?;
    file.write_all(&54u32.to_le_bytes())?;
    file.write_all(&40u32.to_le_bytes())?;
    file.write_all(&(width as i32).to_le_bytes())?;
    file.write_all(&(-(height as i32)).to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&24u16.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;
    file.write_all(&(pixel_bytes as u32).to_le_bytes())?;
    file.write_all(&[0; 16])?;
    let padding = [0u8; 3];
    for y in 0..height {
        for x in 0..width {
            let color = image.pixels[y * width + x];
            file.write_all(&[color.b(), color.g(), color.r()])?;
        }
        let pad = (4 - (width * 3) % 4) % 4;
        file.write_all(&padding[..pad])?;
    }
    file.sync_all()
}

fn render_fields(
    ui: &mut egui::Ui,
    fields: &serde_json::Map<String, serde_json::Value>,
    language: Language,
) {
    if fields.is_empty() {
        ui.label(if language == Language::English {
            "No inventory or additional fields are available."
        } else {
            "Nenhum inventário ou campo adicional disponível."
        });
        return;
    }
    for (key, value) in fields {
        render_value(ui, key, value, 0, language);
    }
}

fn render_value(
    ui: &mut egui::Ui,
    key: &str,
    value: &serde_json::Value,
    depth: usize,
    language: Language,
) {
    match value {
        serde_json::Value::Object(object) if !object.is_empty() => {
            egui::CollapsingHeader::new(key)
                .id_salt((key, depth))
                .default_open(depth == 0)
                .show(ui, |ui| {
                    for (child_key, child_value) in object {
                        render_value(ui, child_key, child_value, depth + 1, language);
                    }
                });
        }
        serde_json::Value::Array(array) if !array.is_empty() => {
            let label = if language == Language::English {
                format!("{key} · {} items", array.len())
            } else {
                format!("{key} · {} itens", array.len())
            };
            egui::CollapsingHeader::new(label)
                .id_salt((key, depth))
                .show(ui, |ui| {
                    for (index, child_value) in array.iter().enumerate() {
                        render_value(ui, &format!("[{index}]"), child_value, depth + 1, language);
                    }
                });
        }
        _ => {
            let rendered = match value {
                serde_json::Value::String(text) => text.clone(),
                serde_json::Value::Null => "—".to_owned(),
                other => other.to_string(),
            };
            ui.horizontal_wrapped(|ui| {
                ui.weak(format!("{key}:"));
                ui.label(rendered);
            });
        }
    }
}

#[cfg(test)]
mod scenario_lifecycle_tests {
    use super::*;
    #[test]
    fn rendering_family_and_tier_filters_preserves_mixed_tier_cart_and_comparison() {
        use crate::scenario_ui::{AllocationChoice, ScenarioForm, TierFilter};
        use grapevine_workbench::pricing::ClanRelation;
        let mut form = ScenarioForm::default();
        form.allocation.profile = true;
        for (id, relation) in [
            (
                "met.lotn-revised.animalism.basic.feral_whispers",
                ClanRelation::InClan,
            ),
            (
                "met.lotn-revised.animalism.intermediate.quell_the_beast",
                ClanRelation::OutOfClan,
            ),
            (
                "met.lotn-revised.animalism.advanced.drawing_out_the_beast",
                ClanRelation::OutOfClan,
            ),
        ] {
            form.allocation.choices.insert(
                id.into(),
                AllocationChoice {
                    selected: true,
                    relation: Some(relation),
                },
            );
        }
        form.comparisons[0] = Some(form.allocation.clone());
        let context = egui::Context::default();
        for tier in [
            TierFilter::Basic,
            TierFilter::Intermediate,
            TierFilter::Advanced,
            TierFilter::All,
        ] {
            form.tier = tier;
            form.family = 1;
            let mut output = context.run_ui(Default::default(), |ui| {
                crate::scenario_ui::render(
                    ui,
                    &mut form,
                    "Vampire",
                    Some("40"),
                    Language::Portuguese,
                    &[],
                )
            });
            output.textures_delta.clear();
            let quote = form.allocation.evaluate("Vampire", Some("40")).unwrap();
            assert_eq!(quote.items.len(), 3);
            assert_eq!(quote.total_spend, "20");
            assert_eq!(quote.projected.as_deref(), Some("20"));
            assert_eq!(
                form.comparisons[0]
                    .as_ref()
                    .unwrap()
                    .choices
                    .iter()
                    .filter(|(_, choice)| choice.selected)
                    .count(),
                3
            );
        }
    }
    #[test]
    fn playtime_review_retains_language_and_close_state_and_clears_for_player() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("playtime-state.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        lib.create_character("Vampire", "Synthetic", 0, Role::Storyteller)
            .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.open_playtime();
        assert!(app.playtime_open);
        app.playtime_form.input.start_month = "2026-01".into();
        app.playtime_form.input.end_month = "2026-02".into();
        app.playtime_form.input.home_chronicle = "PRIVATE_CHRONICLE".into();
        app.playtime_form.input.review_evidence = "PRIVATE_EVIDENCE".into();
        let before = app.playtime_form.input.clone();
        app.language = Language::Portuguese;
        app.playtime_open = false;
        app.open_playtime();
        assert_eq!(app.playtime_form.input, before);
        app.change_route(WorkArea::Player);
        assert!(!app.playtime_open);
        assert!(app.playtime_target.is_none());
        assert!(app.playtime_form.input.home_chronicle.is_empty());
        assert!(app.playtime_form.input.review_evidence.is_empty());
        assert!(app.playtime_form.report.is_none());
        app.open_playtime();
        assert!(!app.playtime_open);
    }
    #[test]
    fn policy_file_failure_preserves_form_and_player_cannot_load_or_save() {
        use grapevine_workbench::{pricing::PricingProfile, purchases::TeacherRule};
        let dir = tempfile::TempDir::new().unwrap();
        let mut app = InconnuApp::new(None);
        app.route = WorkArea::Storyteller;
        app.purchase_form.policy_name = "Synthetic policy".into();
        let r = &mut app.purchase_form.input;
        r.profile = Some(PricingProfile::MetLawsOfTheNightRevised);
        r.standard_cost_confirmed = true;
        r.teacher_rule = Some(TeacherRule::Required);
        r.policy_version = "v1".into();
        r.policy_reference = "Synthetic policy source".into();
        r.approval_reference = "Private approval".into();
        r.teacher_evidence = "Private teacher".into();
        r.eligibility_evidence = "Private eligibility".into();
        let path = dir.path().join("example.gvpolicy");
        app.save_policy_path(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(!text.contains("Private"));
        assert!(app.save_policy_path(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let bad = dir.path().join("bad.gvpolicy");
        std::fs::write(&bad, b"{}").unwrap();
        let before = app.purchase_form.input.clone();
        assert!(app.load_policy_path(&bad).is_err());
        assert_eq!(app.purchase_form.input, before);
        app.load_policy_path(&path).unwrap();
        assert!(app.purchase_form.input.approval_reference.is_empty());
        assert!(app.purchase_form.dirty);
        app.language = Language::Portuguese;
        assert_eq!(app.purchase_form.policy_name, "Synthetic policy");
        app.route = WorkArea::Player;
        assert!(app.load_policy_path(&path).is_err());
        assert!(
            app.save_policy_path(&dir.path().join("denied.gvpolicy"))
                .is_err()
        );
        assert!(!dir.path().join("denied.gvpolicy").exists());
    }
    #[test]
    fn saved_advancement_reopens_in_builder_applies_only_plan_and_clears_private_state() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("saved-plan.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let id = lib
            .create_character("Vampire", "Synthetic", 0, Role::Storyteller)
            .unwrap();
        lib.initialize_xp(
            &id,
            &grapevine_workbench::xp::Baseline {
                day: "2026-01-01".into(),
                earned: "20.25".into(),
                unspent: "20.25".into(),
                actor: "Synthetic ST".into(),
                reason: "Synthetic opening".into(),
                evidence: "Synthetic fixture".into(),
            },
            1,
            Role::Storyteller,
        )
        .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path.clone()));
        app.change_route(WorkArea::Storyteller);
        app.open_purchases();
        for second in [false, true] {
            app.purchase_form.input = smoke_batch_request(second);
            app.purchase_form.dirty = true;
            app.purchase_action(crate::purchase_ui::Action::AddToPlan);
        }
        app.purchase_action(crate::purchase_ui::Action::SavePlanDraft);
        assert_eq!(app.route, WorkArea::Builder);
        assert!(app.advancement.open);
        assert!(!app.purchase_form.dirty);
        let draft_id = app.selected_id.clone().unwrap();
        assert_eq!(
            app.library
                .as_ref()
                .unwrap()
                .xp_view(&id, Role::Storyteller)
                .unwrap()
                .unspent
                .as_deref(),
            Some("20.25")
        );
        drop(app);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Builder);
        app.select_record(draft_id.clone());
        app.open_advancement_draft();
        let context = egui::Context::default();
        for _ in 0..2 {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                app.render_advancement_draft(ui.ctx())
            });
            output.textures_delta.clear();
        }
        assert!(probe(&context, "advancement_draft_prepare").is_some());
        app.prepare_advancement_draft();
        let review = app.advancement.review.clone().unwrap();
        app.language = Language::Portuguese;
        assert_eq!(app.advancement.review.as_ref(), Some(&review));
        app.apply_advancement_draft();
        assert!(app.advancement.review.as_ref().unwrap().applied);
        assert_eq!(
            app.library
                .as_ref()
                .unwrap()
                .xp_view(&id, Role::Storyteller)
                .unwrap()
                .unspent
                .as_deref(),
            Some("13.25")
        );
        let revision = app.library.as_ref().unwrap().revision().unwrap();
        app.apply_advancement_draft();
        assert_eq!(app.library.as_ref().unwrap().revision().unwrap(), revision);
        app.perform_action(PendingAction::Route(WorkArea::Player));
        assert!(!app.advancement.open);
        assert!(app.advancement.review.is_none());
        assert!(app.advancement.target.is_none());
        app.open_advancement_draft();
        assert!(!app.advancement.open);
    }
    #[test]
    fn advancement_queue_retains_language_and_navigation_requires_fresh_xp_review() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("advancement.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        let id = lib
            .create_character("Vampire", "Synthetic", 0, Role::Storyteller)
            .unwrap();
        lib.initialize_xp(
            &id,
            &grapevine_workbench::xp::Baseline {
                day: "2026-01-01".into(),
                earned: "20.25".into(),
                unspent: "20.25".into(),
                actor: "Synthetic ST".into(),
                reason: "Synthetic opening".into(),
                evidence: "Synthetic fixture".into(),
            },
            1,
            Role::Storyteller,
        )
        .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.open_purchases();
        for second in [false, true] {
            app.purchase_form.input = smoke_batch_request(second);
            app.purchase_form.dirty = true;
            app.purchase_action(crate::purchase_ui::Action::AddToPlan);
            assert!(app.purchase_form.input.power_id.is_empty());
            assert!(app.purchase_form.input.approval_reference.is_empty());
            assert!(app.purchase_form.input.teacher_evidence.is_empty());
        }
        assert_eq!(app.purchase_form.plan.len(), 2);
        let quote = app.purchase_form.plan_quote.clone().unwrap();
        app.language = Language::Portuguese;
        assert_eq!(app.purchase_form.plan_quote.as_ref(), Some(&quote));
        app.request_action(PendingAction::Route(WorkArea::Player));
        assert!(app.pending_action.is_some());
        app.pending_action = None;
        assert_eq!(app.purchase_form.plan.len(), 2);
        let lib = app.library.as_mut().unwrap();
        lib.post_xp(
            &id,
            &grapevine_workbench::xp::XpCommand {
                id: uuid::Uuid::new_v4().to_string(),
                kind: grapevine_workbench::xp::XpKind::Award,
                amount: "2".into(),
                day: "2026-01-02".into(),
                actor: "Synthetic ST".into(),
                reason: "New award".into(),
                evidence: "Synthetic attendance".into(),
                reversal_of: None,
                cost: None,
            },
            lib.revision().unwrap(),
            Role::Storyteller,
        )
        .unwrap();
        app.purchase_action(crate::purchase_ui::Action::CommitPlan);
        assert_eq!(app.purchase_form.plan.len(), 2);
        assert!(app.purchase_form.message.is_some());
        app.purchase_action(crate::purchase_ui::Action::ReviewPlan);
        assert_eq!(
            app.purchase_form
                .plan_quote
                .as_ref()
                .unwrap()
                .before_unspent,
            "22.25"
        );
        app.purchase_action(crate::purchase_ui::Action::CommitPlan);
        assert!(app.purchase_form.plan.is_empty());
        assert!(!app.purchase_form.dirty);
        assert_eq!(app.purchase_form.receipts.len(), 2);
        assert_eq!(
            app.library
                .as_ref()
                .unwrap()
                .xp_view(&id, Role::Storyteller)
                .unwrap()
                .unspent
                .as_deref(),
            Some("15.25")
        );
        app.perform_action(PendingAction::Route(WorkArea::Player));
        assert!(app.purchase_form.plan.is_empty());
        assert!(app.purchase_form.plan_quote.is_none());
        app.open_purchases();
        assert!(!app.purchase_open);
    }
    #[test]
    fn purchase_draft_navigation_can_be_cancelled_and_player_cannot_open_private_form() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("purchase-form.gvlib");
        let mut lib = Library::create(&path, "Synthetic").unwrap();
        lib.create_character("Vampire", "Synthetic", 0, Role::Storyteller)
            .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.open_purchases();
        assert!(app.purchase_open);
        app.purchase_form.input.reason = "Unposted purchase draft".into();
        app.purchase_form.dirty = true;
        app.request_action(PendingAction::Route(WorkArea::Player));
        assert_eq!(app.route, WorkArea::Storyteller);
        assert!(app.pending_action.is_some());
        app.pending_action = None;
        assert_eq!(app.purchase_form.input.reason, "Unposted purchase draft");
        app.language = Language::Portuguese;
        assert_eq!(app.purchase_form.input.reason, "Unposted purchase draft");
        app.perform_action(PendingAction::Route(WorkArea::Player));
        assert!(app.purchase_target.is_none());
        assert!(!app.purchase_form.dirty);
        assert!(app.purchase_form.receipts.is_empty());
        app.open_purchases();
        assert!(!app.purchase_open);
    }

    #[test]
    fn scenario_freezes_committed_state_and_requires_explicit_rebase_after_external_edit() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("scenario.gvlib");
        let mut lib = Library::create(&path, "Synthetic scenario").unwrap();
        let id = lib
            .create_character("Vampire", "First", 0, Role::Storyteller)
            .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path.clone()));
        app.change_route(WorkArea::Storyteller);
        app.select_record(id);
        app.open_scenarios();
        assert!(app.scenario_open);
        assert!(!app.scenario_is_stale());
        assert!(app.scenario_balance.is_none());
        app.scenario_form.allocation.profile = true;
        app.scenario_form
            .allocation
            .choices
            .entry(
                grapevine_workbench::purchase_preview::AnimalismPower::ALL[0]
                    .stable_id()
                    .into(),
            )
            .or_default()
            .selected = true;
        app.scenario_form.comparisons[0] = Some(app.scenario_form.allocation.clone());
        let old_revision = app.scenario_revision;
        let mut other = Library::open(&path).unwrap();
        other
            .create_character(
                "Vampire",
                "Second",
                other.revision().unwrap(),
                Role::Storyteller,
            )
            .unwrap();
        assert!(app.scenario_is_stale());
        assert_eq!(app.scenario_revision, old_revision);
        app.open_scenarios();
        assert!(!app.scenario_is_stale());
        assert_ne!(app.scenario_revision, old_revision);
        assert!(
            app.scenario_form
                .allocation
                .choices
                .entry(
                    grapevine_workbench::purchase_preview::AnimalismPower::ALL[0]
                        .stable_id()
                        .into()
                )
                .or_default()
                .selected
        );
        assert!(app.scenario_form.comparisons[0].is_some());
        // Switching language does not change source-qualified choices or comparison data.
        app.language = Language::Portuguese;
        assert!(
            app.scenario_form
                .allocation
                .choices
                .entry(
                    grapevine_workbench::purchase_preview::AnimalismPower::ALL[0]
                        .stable_id()
                        .into()
                )
                .or_default()
                .selected
        );
        app.change_route(WorkArea::Player);
        assert!(!app.scenario_open);
        assert!(app.scenario_baseline.is_none());
        assert!(app.scenario_form.comparisons.iter().all(Option::is_none));
        app.open_scenarios();
        assert!(!app.scenario_open);
    }
}

mod session_window;

#[cfg(test)]
mod session_lifecycle_tests {
    use super::*;
    #[test]
    fn closing_and_language_retain_session_draft_navigation_prompts_and_player_clears() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("session-ui.gvlib");
        let mut lib = Library::create(&path, "Synthetic session UI").unwrap();
        lib.create_character("Vampire", "Synthetic", 0, Role::Storyteller)
            .unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.open_sessions();
        assert!(app.session_open);
        app.session_form.request.title = "Unposted title".into();
        app.session_form.dirty = true;
        let id = app.session_form.request.id.clone();
        app.session_open = false;
        app.language = Language::Portuguese;
        app.open_sessions();
        assert_eq!(app.session_form.request.title, "Unposted title");
        assert_eq!(app.session_form.request.id, id);
        assert_eq!(app.session_form.document_language, Language::English);
        app.request_action(PendingAction::Route(WorkArea::Player));
        assert!(matches!(
            app.pending_action,
            Some(PendingAction::Route(WorkArea::Player))
        ));
        assert_eq!(app.route, WorkArea::Storyteller);
        app.pending_action = None;
        app.change_route(WorkArea::Player);
        assert!(
            !app.session_open
                && app.session_form.quote.is_none()
                && app.session_form.receipts.is_empty()
                && !app.session_form.dirty
        );
        assert!(app.session_form.request.title.is_empty());
    }
}

mod points_window;

#[cfg(test)]
mod points_lifecycle_tests {
    use super::*;
    #[test]
    fn creating_player_preserves_unposted_entry_and_player_route_clears_private_points() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("points-ui.gvlib");
        let lib = Library::create(&path, "Synthetic points UI").unwrap();
        drop(lib);
        let mut app = InconnuApp::new(Some(path));
        app.change_route(WorkArea::Storyteller);
        app.open_points();
        app.points_form.name = "New owner".into();
        app.points_form.actor = "Unposted recorder".into();
        app.points_form.dirty = true;
        app.create_points_player();
        assert!(app.summaries.is_empty());
        assert_eq!(app.points_form.actor, "Unposted recorder");
        app.points_form.actor.clear();
        app.create_points_player();
        assert!(app.points_target.is_some());
        app.points_form.amount = "2".into();
        app.points_form.dirty = true;
        let target = app.points_target.clone();
        app.points_open = false;
        app.language = Language::Portuguese;
        app.open_points();
        assert_eq!(app.points_target, target);
        assert_eq!(app.points_form.amount, "2");
        app.change_route(WorkArea::Player);
        assert!(
            !app.points_open
                && app.points_target.is_none()
                && app.points_ledger.is_none()
                && app.points_form.selected_player.is_none()
                && !app.points_form.dirty
        );
    }
}

mod world_window;

mod plot_window;

mod story_window;

mod association_window;

mod operations_window;

mod profile_window;

mod search_window;

mod boon_window;

mod action_context;

mod advancement_window;
mod chronicle_window;
mod creation_sources_ui;
mod creation_window;
mod met_reference_window;
mod published_window;
mod rumor_audience_window;
mod support_window;
mod trait_advancement_window;

#[cfg(test)]
mod smoke_input_tests {
    use super::*;

    #[test]
    fn smoke_input_ignores_desktop_text_without_changing_normal_input() {
        let ctx = egui::Context::default();
        let mut app = InconnuApp::new(None);
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Text("desktop".into()));
        app.pending_raw_events
            .push(egui::Event::Text("injected".into()));
        eframe::App::raw_input_hook(&mut app, &ctx, &mut input);
        assert_eq!(input.events.len(), 2);

        let dir = tempfile::tempdir().unwrap();
        app = InconnuApp::new_smoke(dir.path().join("absent.gvlib"), dir.path().into());
        input.events = vec![egui::Event::Text("desktop".into())];
        app.pending_raw_events
            .push(egui::Event::Text("injected".into()));
        eframe::App::raw_input_hook(&mut app, &ctx, &mut input);
        assert!(matches!(&input.events[..],[egui::Event::Text(text)] if text=="injected"));
        assert!(input.focused);
    }
}

mod character_lifecycle_window;

mod mixed_advancement_window;

fn smoke_mixed_fixture(
    path: &std::path::Path,
) -> (String, Vec<grapevine_workbench::mixed_advancement::Request>) {
    use grapevine_workbench::{
        mixed_advancement::Request,
        pricing::PricingProfile,
        trait_advancement::{Request as Advance, Target},
        trait_pricing::Category,
        xp::Baseline,
    };
    let mut lib = Library::create(path, "Synthetic mixed advancement").unwrap();
    let id = lib
        .create_character("Vampire", "Synthetic mixed character", 0, Role::Storyteller)
        .unwrap();
    let mut edit = lib.sheet_edit(&id, Role::Storyteller).unwrap();
    let mut list = grapevine_workbench::sheet::empty_list("AbilityList");
    list["Traits"] =
        serde_json::json!([{"Name":"Academics","Value":"4","Note":"PRIVATE_MIXED_TRAIT"}]);
    edit.lists.insert("AbilityList".into(), list);
    edit.visibility.insert("AbilityList".into(), true);
    lib.save_sheet(&id, &edit, lib.revision().unwrap(), Role::Storyteller)
        .unwrap();
    lib.initialize_xp(
        &id,
        &Baseline {
            day: "2026-01-01".into(),
            earned: "20".into(),
            unspent: "20".into(),
            actor: "PRIVATE_MIXED_ACTOR".into(),
            reason: "Opening".into(),
            evidence: "Synthetic".into(),
        },
        lib.revision().unwrap(),
        Role::Storyteller,
    )
    .unwrap();
    let a = Advance {
        id: uuid::Uuid::new_v4().to_string(),
        category: Category::Ability,
        target: Target::Trait {
            field: "AbilityList".into(),
            index: Some(0),
            name: "Academics".into(),
        },
        profile: Some(PricingProfile::MetLawsOfTheNightRevised),
        show_on_player_sheet: true,
        day: "2026-01-02".into(),
        actor: "PRIVATE_MIXED_ACTOR".into(),
        reason: "PRIVATE_MIXED_REASON".into(),
        policy_version: "Synthetic v1".into(),
        review_reference: "PRIVATE_MIXED_REVIEW".into(),
        review_confirmed: true,
    };
    let mut later = a.clone();
    later.id = uuid::Uuid::new_v4().to_string();
    let mut power = smoke_batch_request(true);
    power.actor = "PRIVATE_MIXED_ACTOR".into();
    power.approval_reference = "PRIVATE_MIXED_APPROVAL".into();
    (
        id,
        vec![
            Request::Trait(Box::new(a)),
            Request::Power(Box::new(power)),
            Request::Trait(Box::new(later)),
        ],
    )
}

fn scroll_detail_to(events: &mut Vec<egui::Event>, ctx: &egui::Context, key: &str) {
    let delta = match (probe(ctx, key), probe(ctx, "record_detail_scroll")) {
        (Some(target), Some(view)) if target.min.y < view.min.y + 4.0 => {
            (view.min.y - target.min.y + 8.0).min(80.0)
        }
        (Some(target), Some(view)) if target.max.y > view.max.y - 4.0 => {
            -(target.max.y - view.max.y + 8.0).min(80.0)
        }
        (Some(_), Some(_)) => 0.0,
        _ => -40.0,
    };
    if delta != 0.0 {
        scroll_detail(events, ctx, delta);
    }
}

mod legacy_profile_window;

fn smoke_legacy_profile_fixture(path: &std::path::Path) {
    let source = path.with_extension("gex");
    std::fs::write(&source,r#"<?xml version="1.0"?><grapevine version="3" chronicle="Synthetic legacy chronicle" website="https://example.invalid/game" email="staff@example.invalid[ST]PRIVATE_DEFAULT[/ST]" phone="555-0100" usualtime="Saturday" stcommentstart="[{" stcommentend="}]">
    <usualplace>Community hall</usualplace><description>Legacy description[{PRIVATE_CUSTOM}]</description><vampire name="Synthetic retained character" id="legacy-profile-character"/></grapevine>"#).unwrap();
    let mut lib = Library::create(path, "Current private chronicle").unwrap();
    lib.save_chronicle_profile(
        &grapevine_workbench::chronicle::Profile {
            title: "Current private chronicle".into(),
            email: "old@example.invalid".into(),
            house_rules: "Local rules retained".into(),
            private_notes: "PRIVATE_LEGACY_NOTES".into(),
            ..Default::default()
        },
        0,
        Role::Storyteller,
    )
    .unwrap();
    let p = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    lib.commit_import(&p, lib.revision().unwrap(), false, Role::Storyteller)
        .unwrap();
}
fn smoke_legacy_profile_request(
    source_hash: String,
) -> grapevine_workbench::legacy_profile::Request {
    grapevine_workbench::legacy_profile::Request {
        source_hash,
        selected: grapevine_workbench::legacy_profile::Field::ALL.to_vec(),
        day: "2026-11-15".into(),
        actor: "PRIVATE_LEGACY_REVIEWER".into(),
        reason: "PRIVATE_LEGACY_REASON".into(),
        review_reference: "PRIVATE_LEGACY_REFERENCE".into(),
        confirmed: true,
        confirm_overwrite: true,
        confirm_publication: true,
        ..Default::default()
    }
}

mod recent_libraries_window;

mod legacy_calendar_window;

fn smoke_legacy_calendar_fixture(path: &std::path::Path) {
    let source = path.with_extension("gex");
    std::fs::write(&source,r#"<grapevine version="3"><vampire name="Synthetic calendar character" id="calendar-character"/><calendar><game date="2026-12-01" time="7pm"><place>PRIVATE_CALENDAR_PLACE</place><notes>PRIVATE_CALENDAR_NOTES</notes></game><game date="12/01/2026" time="later"><place>Other private place</place><notes>Separate same-day source entry</notes></game></calendar></grapevine>"#).unwrap();
    let mut lib = Library::create(path, "Synthetic calendar chronicle").unwrap();
    let p = lib
        .preview_import(&source, "windows-1252", Role::Storyteller)
        .unwrap();
    lib.commit_import(&p, 0, false, Role::Storyteller).unwrap();
    let id = lib
        .summaries(Role::Storyteller)
        .unwrap()
        .into_iter()
        .find(|s| s.kind == "Vampire")
        .unwrap()
        .id;
    lib.initialize_xp(
        &id,
        &grapevine_workbench::xp::Baseline {
            day: "2026-11-15".into(),
            earned: "10".into(),
            unspent: "10".into(),
            actor: "Synthetic ST".into(),
            reason: "Synthetic opening".into(),
            evidence: "Synthetic fixture".into(),
        },
        lib.revision().unwrap(),
        Role::Storyteller,
    )
    .unwrap();
}
fn smoke_legacy_calendar_request(hash: String) -> grapevine_workbench::legacy_calendar::Request {
    use grapevine_workbench::legacy_calendar::{Choice, Request};
    Request {
        source_hash: hash,
        choices: (0..2)
            .map(|index| Choice {
                entry_index: index,
                day: "2026-12-01".into(),
                title: format!("Synthetic imported date {}", index + 1),
            })
            .collect(),
        review_day: "2026-11-15".into(),
        actor: "PRIVATE_CALENDAR_ACTOR".into(),
        reason: "PRIVATE_CALENDAR_REASON".into(),
        review_reference: "PRIVATE_CALENDAR_REFERENCE".into(),
        confirmed: true,
        confirm_overlap: true,
        ..Default::default()
    }
}
