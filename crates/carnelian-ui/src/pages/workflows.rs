//! Workflow builder page — list, filter, create, edit, execute workflows
//! with a visual node-based builder (drag-and-drop nodes, connector-based
//! dependency linking), real-time execution monitoring via WebSocket events,
//! and a history panel for past workflow runs.

use std::collections::HashMap;

use carnelian_common::types::{
    CreateWorkflowRequest, EventType, ExecuteWorkflowRequest, SkillDetail, StepResultDetail,
    TaskDetail, UpdateWorkflowRequest, WorkflowDetail, WorkflowExecutionResponse, WorkflowStepDef,
};
use dioxus::prelude::*;
use serde_json::json;
use uuid::Uuid;

use crate::store::EventStreamStore;

// =============================================================================
// MAIN PAGE COMPONENT
// =============================================================================

/// Workflows page with dual-view: list and builder.
#[component]
pub fn Workflows() -> Element {
    let store = use_context::<EventStreamStore>();

    // ── Data fetching ────────────────────────────────────────
    let mut refresh = use_signal(|| 0_u64);

    let workflows_resource = use_resource(move || async move {
        let _ = refresh();
        crate::api::list_workflows(false)
            .await
            .map(|r| r.workflows)
            .unwrap_or_default()
    });

    // Auto-refresh every 5 seconds.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            refresh += 1;
        }
    });

    // Trigger refresh on workflow events from WebSocket.
    use_effect(move || {
        let events = store.events.read();
        if let Some(last) = events.back() {
            match &last.event_type {
                EventType::WorkflowCreated
                | EventType::WorkflowUpdated
                | EventType::WorkflowDeleted
                | EventType::WorkflowExecutionStarted
                | EventType::WorkflowExecutionCompleted
                | EventType::WorkflowExecutionFailed => {
                    refresh += 1;
                }
                _ => {}
            }
        }
    });

    // ── Local UI state ───────────────────────────────────────
    let mut view_mode = use_signal(|| "list".to_string());
    let mut filter_status = use_signal(|| "All".to_string());
    let mut filter_search = use_signal(String::new);
    let sort_col = use_signal(|| "name".to_string());
    let sort_asc = use_signal(|| true);
    let mut show_create = use_signal(|| false);
    let mut show_delete_confirm = use_signal(|| Option::<Uuid>::None);
    let mut execution_view = use_signal(|| Option::<WorkflowDetail>::None);
    let mut builder_workflow = use_signal(|| Option::<WorkflowDetail>::None);
    let mut show_history = use_signal(|| false);

    // ── Derived: filtered + sorted ───────────────────────────
    let workflows_read = workflows_resource.read();
    let all_workflows: Vec<WorkflowDetail> = (*workflows_read)
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);

    let filtered = filter_workflows(&all_workflows, &filter_status.read(), &filter_search.read());
    let sorted = sort_workflows(filtered, &sort_col.read(), *sort_asc.read());

    rsx! {
        div { class: "page-panel panel-page",
            // ── Filter bar ───────────────────────────────────
            div { class: "filter-bar",
                select {
                    class: "filter-select",
                    aria_label: "Filter by status",
                    value: "{filter_status}",
                    onchange: move |e| filter_status.set(e.value()),
                    option { value: "All", "All Statuses" }
                    option { value: "enabled", "Enabled" }
                    option { value: "disabled", "Disabled" }
                }
                input {
                    class: "filter-input",
                    r#type: "text",
                    placeholder: "Search workflows\u{2026}",
                    aria_label: "Search workflows",
                    value: "{filter_search}",
                    oninput: move |e| filter_search.set(e.value()),
                }
                div { class: "filter-bar-actions",
                    button {
                        class: if *view_mode.read() == "list" { "btn-primary btn-sm" } else { "btn-secondary btn-sm" },
                        onclick: move |_| { view_mode.set("list".into()); show_history.set(false); },
                        "List View"
                    }
                    button {
                        class: if *view_mode.read() == "builder" { "btn-primary btn-sm" } else { "btn-secondary btn-sm" },
                        onclick: move |_| { view_mode.set("builder".into()); show_history.set(false); },
                        "Builder View"
                    }
                    button {
                        class: if *show_history.read() { "btn-primary btn-sm" } else { "btn-secondary btn-sm" },
                        onclick: move |_| {
                            let toggled = !*show_history.read();
                            show_history.set(toggled);
                            if toggled { view_mode.set("list".into()); }
                        },
                        "History"
                    }
                    button {
                        class: "btn-primary btn-sm",
                        onclick: move |_| show_create.set(true),
                        "+ Create Workflow"
                    }
                }
            }

            // ── View content ─────────────────────────────────
            if *show_history.read() {
                // ── History Panel ────────────────────────────
                WorkflowHistoryPanel {}
            } else if *view_mode.read() == "list" {
                // ── List View ────────────────────────────────
                if workflows_read.is_none() {
                    div { class: "state-message",
                        div { class: "spinner" }
                        span { "Loading workflows\u{2026}" }
                    }
                } else if sorted.is_empty() {
                    div { class: "state-message",
                        span { class: "state-icon", "\u{1F504}" }
                        span { "No workflows found. Create one to get started." }
                    }
                } else {
                    div { class: "panel-scroll",
                        table { class: "data-table",
                            thead {
                                tr {
                                    { sortable_th("Name", "name", &sort_col, &sort_asc) }
                                    { sortable_th("Description", "description", &sort_col, &sort_asc) }
                                    th { "Steps" }
                                    { sortable_th("Status", "enabled", &sort_col, &sort_asc) }
                                    { sortable_th("Created", "created_at", &sort_col, &sort_asc) }
                                    { sortable_th("Updated", "updated_at", &sort_col, &sort_asc) }
                                    th { "Actions" }
                                }
                            }
                            tbody {
                                for wf in sorted {
                                    { render_workflow_row(
                                        wf,
                                        &mut builder_workflow,
                                        &mut view_mode,
                                        &mut execution_view,
                                        &mut show_delete_confirm,
                                    ) }
                                }
                            }
                        }
                    }
                }
            } else {
                // ── Builder View ─────────────────────────────
                WorkflowBuilderView {
                    workflow: builder_workflow.read().clone(),
                    on_save: move |()| {
                        refresh += 1;
                        view_mode.set("list".into());
                        builder_workflow.set(None);
                    },
                    on_cancel: move |()| {
                        view_mode.set("list".into());
                        builder_workflow.set(None);
                    },
                }
            }

            // ── Modals ───────────────────────────────────────
            if *show_create.read() {
                CreateWorkflowModal {
                    on_close: move || show_create.set(false),
                    on_created: move || {
                        show_create.set(false);
                        refresh += 1;
                    },
                }
            }

            if let Some(wf_id) = *show_delete_confirm.read() {
                DeleteConfirmModal {
                    workflow_id: wf_id,
                    on_close: move || show_delete_confirm.set(None),
                    on_deleted: move || {
                        show_delete_confirm.set(None);
                        refresh += 1;
                    },
                }
            }

            if let Some(wf) = &*execution_view.read() {
                WorkflowExecutionModal {
                    workflow: wf.clone(),
                    on_close: move || execution_view.set(None),
                }
            }
        }
    }
}

// =============================================================================
// HELPERS — filter, sort, sortable header
// =============================================================================

fn filter_workflows<'a>(
    workflows: &'a [WorkflowDetail],
    status: &str,
    search: &str,
) -> Vec<&'a WorkflowDetail> {
    let search_lower = search.to_lowercase();
    workflows
        .iter()
        .filter(|w| match status {
            "enabled" => w.enabled,
            "disabled" => !w.enabled,
            _ => true,
        })
        .filter(|w| {
            if search_lower.is_empty() {
                return true;
            }
            w.name.to_lowercase().contains(&search_lower)
                || w.description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&search_lower)
        })
        .collect()
}

fn sort_workflows<'a>(
    mut workflows: Vec<&'a WorkflowDetail>,
    col: &str,
    asc: bool,
) -> Vec<&'a WorkflowDetail> {
    workflows.sort_by(|a, b| {
        let ord = match col {
            "name" => a.name.cmp(&b.name),
            "description" => a
                .description
                .as_deref()
                .unwrap_or("")
                .cmp(b.description.as_deref().unwrap_or("")),
            "enabled" => a.enabled.cmp(&b.enabled),
            "updated_at" => a.updated_at.cmp(&b.updated_at),
            _ => a.created_at.cmp(&b.created_at),
        };
        if asc { ord } else { ord.reverse() }
    });
    workflows
}

fn sortable_th(
    label: &'static str,
    col: &'static str,
    sort_col: &Signal<String>,
    sort_asc: &Signal<bool>,
) -> Element {
    let mut sc = *sort_col;
    let mut sa = *sort_asc;
    let current_col = sort_col.read().clone();
    let current_asc = *sort_asc.read();
    let indicator = if current_col == col {
        if current_asc { "\u{25B2}" } else { "\u{25BC}" }
    } else {
        ""
    };
    rsx! {
        th {
            onclick: move |_| {
                if *sc.read() == col {
                    let old = *sa.read();
                    sa.set(!old);
                } else {
                    sc.set(col.to_string());
                    sa.set(true);
                }
            },
            "{label} "
            span { class: "sort-indicator", "{indicator}" }
        }
    }
}

// =============================================================================
// LIST VIEW — row renderer
// =============================================================================

fn render_workflow_row(
    wf: &WorkflowDetail,
    builder_workflow: &mut Signal<Option<WorkflowDetail>>,
    view_mode: &mut Signal<String>,
    execution_view: &mut Signal<Option<WorkflowDetail>>,
    show_delete_confirm: &mut Signal<Option<Uuid>>,
) -> Element {
    let enabled_badge = if wf.enabled {
        "badge-status badge-enabled"
    } else {
        "badge-status badge-disabled"
    };
    let enabled_label = if wf.enabled { "Enabled" } else { "Disabled" };
    let created = wf.created_at.format("%Y-%m-%d %H:%M").to_string();
    let updated = wf.updated_at.format("%Y-%m-%d %H:%M").to_string();
    let desc = wf.description.as_deref().unwrap_or("\u{2014}");
    let step_count = wf.steps.len();
    let wf_id = wf.workflow_id;
    let wf_clone_edit = wf.clone();
    let wf_clone_exec = wf.clone();
    let mut builder_workflow = *builder_workflow;
    let mut view_mode = *view_mode;
    let mut execution_view = *execution_view;
    let mut show_delete_confirm = *show_delete_confirm;

    rsx! {
        tr {
            td {
                div { "{wf.name}" }
            }
            td { class: "cell-truncate",
                "{desc}"
            }
            td { "{step_count}" }
            td { span { class: "{enabled_badge}", "{enabled_label}" } }
            td { "{created}" }
            td { "{updated}" }
            td {
                button {
                    class: "btn-secondary btn-sm",
                    onclick: move |_| {
                        builder_workflow.set(Some(wf_clone_edit.clone()));
                        view_mode.set("builder".into());
                    },
                    "Edit"
                }
                button {
                    class: "btn-primary btn-sm",
                    onclick: move |_| {
                        execution_view.set(Some(wf_clone_exec.clone()));
                    },
                    "Execute"
                }
                button {
                    class: "btn-danger btn-sm",
                    onclick: move |_| {
                        show_delete_confirm.set(Some(wf_id));
                    },
                    "Delete"
                }
            }
        }
    }
}

// =============================================================================
// VISUAL WORKFLOW BUILDER
// =============================================================================

#[component]
fn WorkflowBuilderView(
    workflow: Option<WorkflowDetail>,
    on_save: EventHandler,
    on_cancel: EventHandler,
) -> Element {
    // ── Skills catalog ───────────────────────────────────────
    let skills_resource = use_resource(|| async {
        crate::api::list_skills()
            .await
            .map(|r| r.skills)
            .unwrap_or_default()
    });

    // ── Builder state ────────────────────────────────────────
    let mut builder_name = use_signal(|| {
        workflow
            .as_ref()
            .map_or_else(String::new, |w| w.name.clone())
    });
    let mut builder_desc = use_signal(|| {
        workflow
            .as_ref()
            .and_then(|w| w.description.clone())
            .unwrap_or_default()
    });
    let mut builder_steps =
        use_signal(|| workflow.as_ref().map_or_else(Vec::new, |w| w.steps.clone()));
    let mut node_positions = use_signal(|| {
        let mut map = HashMap::<String, (f64, f64)>::new();
        if let Some(ref w) = workflow {
            for (i, step) in w.steps.iter().enumerate() {
                map.insert(
                    step.step_id.clone(),
                    (
                        (i as f64 % 3.0).mul_add(220.0, 120.0),
                        (i as f64 / 3.0).floor().mul_add(160.0, 80.0),
                    ),
                );
            }
        }
        map
    });
    let mut selected_step = use_signal(|| Option::<String>::None);
    let mut skill_search = use_signal(String::new);
    let mut validation_errors = use_signal(Vec::<String>::new);
    let mut saving = use_signal(|| false);

    // ── Drag state for node positioning ──────────────────────
    // (step_id, offset_x from node origin, offset_y from node origin)
    let mut dragging_node = use_signal(|| Option::<(String, f64, f64)>::None);
    // ── Connector drag state for dependency linking ──────────
    // Source step_id being connected from (output port)
    let mut connecting_from = use_signal(|| Option::<String>::None);
    // Current mouse position during connector drag (for rubber-band line)
    let mut connector_mouse = use_signal(|| (0.0_f64, 0.0_f64));

    let editing_id = workflow.as_ref().map(|w| w.workflow_id);

    // ── Derived ──────────────────────────────────────────────
    let skills_read = skills_resource.read();
    let all_skills: Vec<SkillDetail> = (*skills_read)
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);

    let skill_search_lower = skill_search.read().to_lowercase();
    let filtered_skills: Vec<&SkillDetail> = all_skills
        .iter()
        .filter(|s| s.enabled)
        .filter(|s| {
            skill_search_lower.is_empty() || s.name.to_lowercase().contains(&skill_search_lower)
        })
        .collect();

    let steps_snapshot = builder_steps.read().clone();
    let positions_snapshot = node_positions.read().clone();
    let selected_id = selected_step.read().clone();

    // Find selected step detail
    let selected_detail: Option<WorkflowStepDef> = selected_id
        .as_ref()
        .and_then(|sid| steps_snapshot.iter().find(|s| &s.step_id == sid).cloned());

    // Connector drag snapshot
    let connecting_from_snap = connecting_from.read().clone();
    let connector_mouse_snap = *connector_mouse.read();

    rsx! {
        div { class: "wf-builder-container",
            // ── Left sidebar: Skills catalog ─────────────────
            div { class: "wf-sidebar",
                h3 { "Skills Catalog" }
                input {
                    class: "filter-input",
                    r#type: "text",
                    placeholder: "Search skills\u{2026}",
                    value: "{skill_search}",
                    oninput: move |e| skill_search.set(e.value()),
                    style: "width:100%; margin-bottom:8px;",
                }
                div { class: "wf-skill-list",
                    for skill in filtered_skills {
                        { render_skill_card(skill, &mut builder_steps, &mut node_positions) }
                    }
                }
            }

            // ── Center: Canvas ───────────────────────────────
            div { class: "wf-canvas-container",
                // Toolbar
                div { class: "wf-toolbar",
                    input {
                        class: "form-input",
                        r#type: "text",
                        placeholder: "Workflow name",
                        value: "{builder_name}",
                        oninput: move |e| builder_name.set(e.value()),
                        style: "flex:1;",
                    }
                    button {
                        class: "btn-secondary btn-sm",
                        onclick: move |_| {
                            let errors = validate_steps(&builder_steps.read());
                            validation_errors.set(errors);
                        },
                        "Validate"
                    }
                    button {
                        class: "btn-secondary btn-sm",
                        onclick: move |_| {
                            auto_layout(&builder_steps.read(), &mut node_positions);
                        },
                        "Auto-Layout"
                    }
                    button {
                        class: "btn-primary btn-sm",
                        disabled: *saving.read(),
                        onclick: {
                            let name = builder_name.read().clone();
                            let desc = builder_desc.read().clone();
                            move |_| {
                                let errors = validate_steps(&builder_steps.read());
                                if !errors.is_empty() {
                                    validation_errors.set(errors);
                                    return;
                                }
                                validation_errors.set(vec![]);
                                saving.set(true);
                                let name = name.clone();
                                let desc = desc.clone();
                                let steps = builder_steps.read().clone();
                                let wf_id = editing_id;
                                spawn(async move {
                                    let result = if let Some(id) = wf_id {
                                        crate::api::update_workflow(id, UpdateWorkflowRequest {
                                            name: Some(name),
                                            description: Some(desc).filter(|d| !d.is_empty()),
                                            steps: Some(steps),
                                        }).await
                                    } else {
                                        crate::api::create_workflow(CreateWorkflowRequest {
                                            name,
                                            description: Some(desc).filter(|d| !d.is_empty()),
                                            steps,
                                        }).await
                                    };
                                    saving.set(false);
                                    match result {
                                        Ok(_) => on_save.call(()),
                                        Err(e) => {
                                            tracing::warn!(error = %e, "Failed to save workflow");
                                            validation_errors.set(vec![e]);
                                        }
                                    }
                                });
                            }
                        },
                        if *saving.read() { "Saving\u{2026}" } else { "Save Workflow" }
                    }
                    button {
                        class: "btn-secondary btn-sm",
                        onclick: move |_| on_cancel.call(()),
                        "Cancel"
                    }
                }

                // Description
                input {
                    class: "form-input",
                    r#type: "text",
                    placeholder: "Description (optional)",
                    value: "{builder_desc}",
                    oninput: move |e| builder_desc.set(e.value()),
                    style: "width:100%; margin-bottom:8px;",
                }

                // Validation errors
                if !validation_errors.read().is_empty() {
                    div { class: "wf-validation-errors",
                        for err in validation_errors.read().iter() {
                            div { class: "wf-validation-error", "{err}" }
                        }
                    }
                }

                // SVG Canvas with drag handlers
                div {
                    class: "wf-canvas",
                    // Global mousemove: update dragging node position or connector rubber-band
                    onmousemove: move |e: Event<MouseData>| {
                        let coords = e.data().client_coordinates();
                        let mx = coords.x;
                        let my = coords.y;
                        // Node drag
                        if let Some((ref step_id, ox, oy)) = *dragging_node.read() {
                            let new_x = (mx - ox).max(0.0);
                            let new_y = (my - oy).max(0.0);
                            let mut pos = node_positions.read().clone();
                            pos.insert(step_id.clone(), (new_x, new_y));
                            node_positions.set(pos);
                        }
                        // Connector drag rubber-band
                        if connecting_from.read().is_some() {
                            connector_mouse.set((mx, my));
                        }
                    },
                    // Global mouseup: finish node drag or connector drop
                    onmouseup: move |e: Event<MouseData>| {
                        // Finish node drag
                        if dragging_node.read().is_some() {
                            dragging_node.set(None);
                        }
                        // Finish connector drag — find target node under cursor
                        let conn_source = connecting_from.read().clone();
                        if let Some(source_id) = conn_source {
                            let coords = e.data().client_coordinates();
                            let mx = coords.x;
                            let my = coords.y;
                            // Hit-test: find node whose bounding box contains (mx, my)
                            let positions = node_positions.read();
                            let mut target_id = None;
                            for (sid, &(nx, ny)) in positions.iter() {
                                if sid != &source_id
                                    && mx >= nx && mx <= nx + NODE_WIDTH
                                    && my >= ny && my <= ny + NODE_HEIGHT
                                {
                                    target_id = Some(sid.clone());
                                    break;
                                }
                            }
                            drop(positions);
                            if let Some(tid) = target_id {
                                // Add dependency: target depends on source
                                let mut steps = builder_steps.read().clone();
                                if let Some(target_step) = steps.iter_mut().find(|s| s.step_id == tid) {
                                    if !target_step.depends_on.contains(&source_id) {
                                        target_step.depends_on.push(source_id);
                                    }
                                }
                                builder_steps.set(steps);
                            }
                            connecting_from.set(None);
                        }
                    },
                    // Cancel drag if mouse leaves canvas
                    onmouseleave: move |_| {
                        dragging_node.set(None);
                        connecting_from.set(None);
                    },
                    svg {
                        width: "100%",
                        height: "500",
                        view_box: "0 0 900 500",
                        // Grid background
                        defs {
                            pattern {
                                id: "grid",
                                width: "20",
                                height: "20",
                                pattern_units: "userSpaceOnUse",
                                path {
                                    d: "M 20 0 L 0 0 0 20",
                                    fill: "none",
                                    stroke: "rgba(255,255,255,0.04)",
                                    stroke_width: "0.5",
                                }
                            }
                        }
                        rect {
                            width: "100%",
                            height: "100%",
                            fill: "url(#grid)",
                        }

                        // Connection lines (dependency arrows)
                        for step in steps_snapshot.iter() {
                            for dep in step.depends_on.iter() {
                                { render_connection(dep, &step.step_id, &positions_snapshot, &mut builder_steps) }
                            }
                        }

                        // Rubber-band connector line during drag
                        if let Some(ref src_id) = connecting_from_snap {
                            { render_rubber_band(src_id, connector_mouse_snap, &positions_snapshot) }
                        }

                        // Nodes with drag + connector ports
                        for step in steps_snapshot.iter() {
                            { render_node(
                                step,
                                &positions_snapshot,
                                selected_id.as_deref() == Some(&step.step_id),
                                &mut selected_step,
                                &mut dragging_node,
                                &mut connecting_from,
                            ) }
                        }
                    }
                }
            }

            // ── Right panel: Step config ─────────────────────
            div { class: "wf-config-panel",
                if let Some(step) = selected_detail {
                    StepConfigPanel {
                        step: step,
                        on_update: move |updated: WorkflowStepDef| {
                            let mut steps = builder_steps.read().clone();
                            if let Some(s) = steps.iter_mut().find(|s| s.step_id == updated.step_id) {
                                *s = updated;
                            }
                            builder_steps.set(steps);
                        },
                        on_delete: move |step_id: String| {
                            // Also remove this step from all depends_on lists
                            let steps: Vec<WorkflowStepDef> = builder_steps
                                .read()
                                .iter()
                                .filter(|s| s.step_id != step_id)
                                .cloned()
                                .map(|mut s| {
                                    s.depends_on.retain(|d| d != &step_id);
                                    s
                                })
                                .collect();
                            builder_steps.set(steps);
                            selected_step.set(None);
                            let mut pos = node_positions.read().clone();
                            pos.remove(&step_id);
                            node_positions.set(pos);
                        },
                    }
                } else {
                    div { class: "wf-config-empty",
                        p { class: "text-secondary", "Select a step on the canvas to configure it, or add a skill from the catalog." }
                        p { class: "text-secondary", style: "margin-top:8px; font-size:11px;",
                            "Drag nodes to reposition. Drag from the bottom port (\u{25CF}) of a node to the top port of another to create a dependency. Click a connection arrow to remove it."
                        }
                    }
                }
            }
        }
    }
}

/// Node dimensions used for hit-testing and layout.
const NODE_WIDTH: f64 = 180.0;
const NODE_HEIGHT: f64 = 50.0;

// =============================================================================
// BUILDER HELPERS
// =============================================================================

fn render_skill_card(
    skill: &SkillDetail,
    builder_steps: &mut Signal<Vec<WorkflowStepDef>>,
    node_positions: &mut Signal<HashMap<String, (f64, f64)>>,
) -> Element {
    let skill_name = skill.name.clone();
    let runtime = skill.runtime.clone();
    let desc = skill.description.clone().unwrap_or_default();
    let mut builder_steps_copy = *builder_steps;
    let mut node_positions_copy = *node_positions;

    let runtime_class = match runtime.as_str() {
        "node" => "wf-runtime-node",
        "python" => "wf-runtime-python",
        _ => "wf-runtime-shell",
    };

    rsx! {
        div {
            class: "wf-skill-card",
            onclick: {
                let skill_name = skill_name;
                move |_| {
                    let step_id = format!("step_{}", Uuid::new_v4().to_string().split('-').next().unwrap_or("x"));
                    let count = builder_steps_copy.read().len();
                    let new_step = WorkflowStepDef {
                        step_id: step_id.clone(),
                        skill_name: skill_name.clone(),
                        input_mapping: None,
                        depends_on: vec![],
                        condition: None,
                        retry_policy: None,
                        continue_on_error: false,
                    };
                    let mut steps = builder_steps_copy.read().clone();
                    steps.push(new_step);
                    builder_steps_copy.set(steps);
                    let mut pos = node_positions_copy.read().clone();
                    pos.insert(
                        step_id,
                        ((count as f64 % 3.0).mul_add(220.0, 120.0), (count as f64 / 3.0).floor().mul_add(160.0, 80.0)),
                    );
                    node_positions_copy.set(pos);
                }
            },
            div { class: "wf-skill-card-name", "{skill_name}" }
            span { class: "badge-status {runtime_class}", "{runtime}" }
            if !desc.is_empty() {
                div { class: "text-secondary", style: "font-size:11px; margin-top:4px;", "{desc}" }
            }
        }
    }
}

fn render_connection(
    from_id: &str,
    to_id: &str,
    positions: &HashMap<String, (f64, f64)>,
    builder_steps: &mut Signal<Vec<WorkflowStepDef>>,
) -> Element {
    let from = positions.get(from_id).copied().unwrap_or((0.0, 0.0));
    let to = positions.get(to_id).copied().unwrap_or((0.0, 0.0));

    let x1 = from.0 + NODE_WIDTH / 2.0;
    let y1 = from.1 + NODE_HEIGHT; // bottom of source node
    let x2 = to.0 + NODE_WIDTH / 2.0;
    let y2 = to.1; // top of target node

    let mid_y = (y1 + y2) / 2.0;
    let d = format!("M {x1} {y1} C {x1} {mid_y}, {x2} {mid_y}, {x2} {y2}");

    // Click-to-remove: clicking the connection removes the dependency
    let dep_from = from_id.to_string();
    let dep_to = to_id.to_string();
    let mut builder_steps = *builder_steps;

    rsx! {
        // Invisible wider path for easier click target
        path {
            d: "{d}",
            fill: "none",
            stroke: "transparent",
            stroke_width: "12",
            cursor: "pointer",
            onclick: move |e: Event<MouseData>| {
                e.stop_propagation();
                let mut steps = builder_steps.read().clone();
                if let Some(target_step) = steps.iter_mut().find(|s| s.step_id == dep_to) {
                    target_step.depends_on.retain(|d| d != &dep_from);
                }
                builder_steps.set(steps);
            },
        }
        path {
            d: "{d}",
            fill: "none",
            stroke: "rgba(74, 144, 226, 0.5)",
            stroke_width: "2",
            stroke_dasharray: "6,3",
            pointer_events: "none",
        }
        // Arrowhead
        circle {
            cx: "{x2}",
            cy: "{y2}",
            r: "4",
            fill: "#4A90E2",
            pointer_events: "none",
        }
    }
}

/// Render a rubber-band line from the source node's output port to the current mouse position.
fn render_rubber_band(
    src_id: &str,
    mouse: (f64, f64),
    positions: &HashMap<String, (f64, f64)>,
) -> Element {
    let from = positions.get(src_id).copied().unwrap_or((0.0, 0.0));
    let x1 = from.0 + NODE_WIDTH / 2.0;
    let y1 = from.1 + NODE_HEIGHT;
    let x2 = mouse.0;
    let y2 = mouse.1;
    let mid_y = (y1 + y2) / 2.0;
    let d = format!("M {x1} {y1} C {x1} {mid_y}, {x2} {mid_y}, {x2} {y2}");

    rsx! {
        path {
            d: "{d}",
            fill: "none",
            stroke: "rgba(74, 144, 226, 0.7)",
            stroke_width: "2",
            stroke_dasharray: "4,4",
            pointer_events: "none",
        }
    }
}

fn render_node(
    step: &WorkflowStepDef,
    positions: &HashMap<String, (f64, f64)>,
    is_selected: bool,
    selected_step: &Signal<Option<String>>,
    dragging_node: &Signal<Option<(String, f64, f64)>>,
    connecting_from: &Signal<Option<String>>,
) -> Element {
    let pos = positions
        .get(&step.step_id)
        .copied()
        .unwrap_or((50.0, 50.0));
    let x = pos.0;
    let y = pos.1;
    let step_id = step.step_id.clone();
    let step_id_drag = step.step_id.clone();
    let step_id_conn = step.step_id.clone();
    let skill_name = step.skill_name.clone();
    let border_color = if is_selected {
        "rgba(74, 144, 226, 0.9)"
    } else {
        "rgba(255, 255, 255, 0.15)"
    };
    let mut selected_step = *selected_step;
    let mut dragging_node = *dragging_node;
    let mut connecting_from = *connecting_from;

    // Port positions
    let input_port_center_x = x + NODE_WIDTH / 2.0;
    let input_port_center_y = y; // top center
    let output_port_center_x = x + NODE_WIDTH / 2.0;
    let output_port_center_y = y + NODE_HEIGHT; // bottom center

    rsx! {
        g {
            // Click to select; mousedown on body to start drag
            onclick: move |_| {
                selected_step.set(Some(step_id.clone()));
            },
            onmousedown: move |e: Event<MouseData>| {
                e.stop_propagation();
                let coords = e.data().client_coordinates();
                let ox = coords.x - x;
                let oy = coords.y - y;
                dragging_node.set(Some((step_id_drag.clone(), ox, oy)));
            },
            cursor: "grab",
            // Node body
            rect {
                x: "{x}",
                y: "{y}",
                width: "{NODE_WIDTH}",
                height: "{NODE_HEIGHT}",
                rx: "8",
                ry: "8",
                fill: "rgba(30, 30, 50, 0.85)",
                stroke: "{border_color}",
                stroke_width: "1.5",
            }
            text {
                x: "{x + NODE_WIDTH / 2.0}",
                y: "{y + 20.0}",
                text_anchor: "middle",
                fill: "#E0E0E0",
                font_size: "12",
                font_weight: "600",
                pointer_events: "none",
                "{skill_name}"
            }
            text {
                x: "{x + NODE_WIDTH / 2.0}",
                y: "{y + 36.0}",
                text_anchor: "middle",
                fill: "#7F8C8D",
                font_size: "10",
                pointer_events: "none",
                "{step.step_id}"
            }
            // Input port (top center) — visual indicator for drop target
            circle {
                cx: "{input_port_center_x}",
                cy: "{input_port_center_y}",
                r: "5",
                fill: "rgba(74, 144, 226, 0.4)",
                stroke: "rgba(74, 144, 226, 0.8)",
                stroke_width: "1",
            }
            // Output port (bottom center) — drag from here to create dependency
            circle {
                cx: "{output_port_center_x}",
                cy: "{output_port_center_y}",
                r: "6",
                fill: "#4A90E2",
                stroke: "rgba(255,255,255,0.3)",
                stroke_width: "1",
                cursor: "crosshair",
                onmousedown: move |e: Event<MouseData>| {
                    e.stop_propagation();
                    connecting_from.set(Some(step_id_conn.clone()));
                },
            }
        }
    }
}

fn validate_steps(steps: &[WorkflowStepDef]) -> Vec<String> {
    let mut errors = Vec::new();

    if steps.is_empty() {
        errors.push("Workflow must have at least one step".into());
        return errors;
    }

    // Check unique step IDs
    let mut seen = std::collections::HashSet::new();
    for step in steps {
        if !seen.insert(&step.step_id) {
            errors.push(format!("Duplicate step ID: {}", step.step_id));
        }
    }

    // Check for empty skill_name
    for step in steps {
        if step.skill_name.trim().is_empty() {
            errors.push(format!("Step '{}' has no skill assigned", step.step_id));
        }
    }

    // Validate input_mapping JSON
    for step in steps {
        if let Some(ref mapping) = step.input_mapping {
            // input_mapping should be a JSON object
            if !mapping.is_object() {
                errors.push(format!(
                    "Step '{}': input_mapping must be a JSON object, got {}",
                    step.step_id,
                    json_type_name(mapping),
                ));
            }
        }
    }

    // Validate condition JSON
    for step in steps {
        if let Some(ref cond) = step.condition {
            // condition should be a JSON object with recognizable fields
            if !cond.is_object() {
                errors.push(format!(
                    "Step '{}': condition must be a JSON object, got {}",
                    step.step_id,
                    json_type_name(cond),
                ));
            }
        }
    }

    // Check dependency references
    let step_ids: std::collections::HashSet<&str> =
        steps.iter().map(|s| s.step_id.as_str()).collect();
    for step in steps {
        for dep in &step.depends_on {
            if !step_ids.contains(dep.as_str()) {
                errors.push(format!(
                    "Step '{}' depends on unknown step '{}'",
                    step.step_id, dep
                ));
            }
            if dep == &step.step_id {
                errors.push(format!("Step '{}' depends on itself", step.step_id));
            }
        }
    }

    // Cycle detection (BFS-based topological sort)
    let mut in_deg: HashMap<&str, usize> = steps.iter().map(|s| (s.step_id.as_str(), 0)).collect();
    for step in steps {
        for _dep in &step.depends_on {
            *in_deg.entry(step.step_id.as_str()).or_insert(0) += 1;
        }
    }
    let mut queue: Vec<&str> = in_deg
        .iter()
        .filter(|&(_, d)| *d == 0)
        .map(|(&id, _)| id)
        .collect();
    let mut visited = 0;
    while let Some(node) = queue.pop() {
        visited += 1;
        for step in steps {
            if step.depends_on.iter().any(|d| d == node) {
                let entry = in_deg.entry(step.step_id.as_str()).or_insert(0);
                *entry = entry.saturating_sub(1);
                if *entry == 0 {
                    queue.push(&step.step_id);
                }
            }
        }
    }
    if visited < steps.len() {
        errors.push("Circular dependency detected in workflow steps".into());
    }

    errors
}

/// Return a human-readable name for a JSON value type.
const fn json_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn auto_layout(steps: &[WorkflowStepDef], positions: &mut Signal<HashMap<String, (f64, f64)>>) {
    // Simple topological layout: steps with no dependencies first
    let mut in_deg: HashMap<&str, usize> = steps.iter().map(|s| (s.step_id.as_str(), 0)).collect();
    for step in steps {
        for _dep in &step.depends_on {
            *in_deg.entry(step.step_id.as_str()).or_insert(0) += 1;
        }
    }

    let mut layers: Vec<Vec<&str>> = Vec::new();
    let mut remaining: Vec<&str> = steps.iter().map(|s| s.step_id.as_str()).collect();
    let mut placed = std::collections::HashSet::new();

    while !remaining.is_empty() {
        let layer: Vec<&str> = remaining
            .iter()
            .filter(|&&id| {
                let step = steps.iter().find(|s| s.step_id == id).unwrap();
                step.depends_on.iter().all(|d| placed.contains(d.as_str()))
            })
            .copied()
            .collect();

        if layer.is_empty() {
            // Remaining steps have unresolvable deps, place them anyway
            layers.push(remaining.clone());
            break;
        }

        for &id in &layer {
            placed.insert(id);
        }
        remaining.retain(|id| !placed.contains(*id));
        layers.push(layer);
    }

    let mut new_pos = HashMap::new();
    for (row, layer) in layers.iter().enumerate() {
        for (col, &step_id) in layer.iter().enumerate() {
            new_pos.insert(
                step_id.to_string(),
                ((col as f64).mul_add(220.0, 80.0), (row as f64).mul_add(120.0, 60.0)),
            );
        }
    }
    positions.set(new_pos);
}

// =============================================================================
// STEP CONFIGURATION PANEL
// =============================================================================

#[component]
fn StepConfigPanel(
    step: WorkflowStepDef,
    on_update: EventHandler<WorkflowStepDef>,
    on_delete: EventHandler<String>,
) -> Element {
    let mut input_mapping_str = use_signal(|| {
        step.input_mapping.as_ref().map_or_else(String::new, |v| {
            serde_json::to_string_pretty(v).unwrap_or_default()
        })
    });
    let mut condition_str = use_signal(|| {
        step.condition.as_ref().map_or_else(String::new, |v| {
            serde_json::to_string_pretty(v).unwrap_or_default()
        })
    });
    let mut continue_on_error = use_signal(|| step.continue_on_error);
    // JSON parse error feedback
    let mut input_mapping_err = use_signal(|| Option::<String>::None);
    let mut condition_err = use_signal(|| Option::<String>::None);

    let step_clone = step.clone();
    let step_id = step_clone.step_id.clone();
    let skill_name = step_clone.skill_name.clone();
    let dep_display: String = if step_clone.depends_on.is_empty() {
        "None".to_string()
    } else {
        step_clone.depends_on.join(", ")
    };

    rsx! {
        div { class: "wf-config-content",
            h3 { "Step: {step_id}" }
            div { class: "text-secondary", style: "margin-bottom:12px;", "Skill: {skill_name}" }

            // Dependencies (read-only — managed via canvas connectors)
            div { class: "form-group",
                label { class: "form-label", "Dependencies (drag connectors on canvas to edit)" }
                div { class: "text-secondary", style: "font-size:12px; padding:4px 0;", "{dep_display}" }
            }

            div { class: "form-group",
                label { class: "form-label", "Input Mapping (JSON)" }
                textarea {
                    class: if input_mapping_err.read().is_some() { "form-textarea wf-field-error" } else { "form-textarea" },
                    style: "font-family: monospace; font-size: 12px; min-height: 80px;",
                    value: "{input_mapping_str}",
                    oninput: {
                        let step_for_closure = step_clone.clone();
                        let cont = *continue_on_error.read();
                        let cond_str = condition_str.read().clone();
                        move |e: Event<FormData>| {
                            let raw = e.value();
                            input_mapping_str.set(raw.clone());
                            let mapping = if raw.trim().is_empty() {
                                input_mapping_err.set(None);
                                None
                            } else {
                                match serde_json::from_str::<serde_json::Value>(&raw) {
                                    Ok(v) => { input_mapping_err.set(None); Some(v) }
                                    Err(err) => {
                                        input_mapping_err.set(Some(format!("Invalid JSON: {err}")));
                                        return; // Don't propagate invalid JSON
                                    }
                                }
                            };
                            let condition = if cond_str.trim().is_empty() { None } else { serde_json::from_str(&cond_str).ok() };
                            let mut updated = step_for_closure.clone();
                            updated.input_mapping = mapping;
                            updated.condition = condition;
                            updated.continue_on_error = cont;
                            on_update.call(updated);
                        }
                    },
                }
                if let Some(err) = &*input_mapping_err.read() {
                    div { class: "wf-field-error-msg", "{err}" }
                }
            }

            div { class: "form-group",
                label { class: "form-label", "Condition (JSON)" }
                textarea {
                    class: if condition_err.read().is_some() { "form-textarea wf-field-error" } else { "form-textarea" },
                    style: "font-family: monospace; font-size: 12px; min-height: 60px;",
                    value: "{condition_str}",
                    oninput: {
                        let step_for_cond = step_clone.clone();
                        let cont = *continue_on_error.read();
                        let map_str = input_mapping_str.read().clone();
                        move |e: Event<FormData>| {
                            let raw = e.value();
                            condition_str.set(raw.clone());
                            let condition = if raw.trim().is_empty() {
                                condition_err.set(None);
                                None
                            } else {
                                match serde_json::from_str::<serde_json::Value>(&raw) {
                                    Ok(v) => { condition_err.set(None); Some(v) }
                                    Err(err) => {
                                        condition_err.set(Some(format!("Invalid JSON: {err}")));
                                        return; // Don't propagate invalid JSON
                                    }
                                }
                            };
                            let mapping = if map_str.trim().is_empty() { None } else { serde_json::from_str(&map_str).ok() };
                            let mut updated = step_for_cond.clone();
                            updated.condition = condition;
                            updated.input_mapping = mapping;
                            updated.continue_on_error = cont;
                            on_update.call(updated);
                        }
                    },
                }
                if let Some(err) = &*condition_err.read() {
                    div { class: "wf-field-error-msg", "{err}" }
                }
            }

            div { class: "form-group",
                label { class: "filter-checkbox",
                    input {
                        r#type: "checkbox",
                        checked: *continue_on_error.read(),
                        onchange: {
                            let step_for_checkbox = step_clone;
                            let map_str = input_mapping_str.read().clone();
                            let cond_str = condition_str.read().clone();
                            move |_| {
                                let new_val = !*continue_on_error.read();
                                continue_on_error.set(new_val);
                                let mut updated = step_for_checkbox.clone();
                                updated.continue_on_error = new_val;
                                updated.input_mapping = if map_str.trim().is_empty() { None } else { serde_json::from_str(&map_str).ok() };
                                updated.condition = if cond_str.trim().is_empty() { None } else { serde_json::from_str(&cond_str).ok() };
                                on_update.call(updated);
                            }
                        },
                    }
                    "Continue on error"
                }
            }

            button {
                class: "btn-danger btn-sm",
                style: "margin-top:12px;",
                onclick: {
                    let sid = step_id;
                    move |_| on_delete.call(sid.clone())
                },
                "Remove Step"
            }
        }
    }
}

// =============================================================================
// EXECUTION MODAL
// =============================================================================

/// Live step status tracked from WebSocket events during execution.
#[derive(Clone, Debug)]
struct LiveStepStatus {
    step_id: String,
    skill_name: String,
    status: String, // "pending", "running", "success", "failed", "skipped"
    duration_ms: u64,
}

#[component]
fn WorkflowExecutionModal(workflow: WorkflowDetail, on_close: EventHandler) -> Element {
    let store = use_context::<EventStreamStore>();

    let mut execution_input = use_signal(|| "{}".to_string());
    let mut input_parse_err = use_signal(|| Option::<String>::None);
    let mut executing = use_signal(|| false);
    let mut execution_result =
        use_signal(|| Option::<Result<WorkflowExecutionResponse, String>>::None);
    // Live step statuses populated from WebSocket events
    let mut live_steps = use_signal(Vec::<LiveStepStatus>::new);
    // Track the correlation_id for this execution to filter events
    let mut exec_correlation = use_signal(|| Option::<String>::None);
    // Overall live status
    let mut live_status = use_signal(|| Option::<String>::None);

    let wf_id = workflow.workflow_id;
    let wf_name = workflow.name.clone();
    let wf_id_str = wf_id.to_string();

    // Subscribe to WebSocket events for real-time step progress
    use_effect(move || {
        let events = store.events.read();
        if let Some(last) = events.back() {
            let payload = &last.payload;
            // Only process events for this workflow
            let event_wf_id = payload
                .get("workflow_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if event_wf_id != wf_id_str {
                return;
            }
            // Optionally filter by correlation_id if we have one
            if let Some(ref corr) = *exec_correlation.read() {
                let event_corr = payload
                    .get("correlation_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !event_corr.is_empty() && event_corr != corr {
                    return;
                }
            }

            match &last.event_type {
                EventType::WorkflowExecutionStarted => {
                    // Capture correlation_id from the start event
                    if let Some(corr) = payload.get("correlation_id").and_then(|v| v.as_str()) {
                        exec_correlation.set(Some(corr.to_string()));
                    }
                    live_status.set(Some("running".to_string()));
                    // Initialize live steps from the workflow definition
                    let initial: Vec<LiveStepStatus> = workflow
                        .steps
                        .iter()
                        .map(|s| LiveStepStatus {
                            step_id: s.step_id.clone(),
                            skill_name: s.skill_name.clone(),
                            status: "pending".to_string(),
                            duration_ms: 0,
                        })
                        .collect();
                    live_steps.set(initial);
                }
                EventType::WorkflowStepCompleted => {
                    let step_id = payload
                        .get("step_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let status = payload
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let duration_ms = payload
                        .get("duration_ms")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0);
                    let skill_name = payload
                        .get("skill_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let mut steps = live_steps.read().clone();
                    if let Some(s) = steps.iter_mut().find(|s| s.step_id == step_id) {
                        s.status = status;
                        s.duration_ms = duration_ms;
                        if !skill_name.is_empty() {
                            s.skill_name = skill_name;
                        }
                    }
                    live_steps.set(steps);
                }
                EventType::WorkflowExecutionCompleted => {
                    live_status.set(Some("success".to_string()));
                }
                EventType::WorkflowExecutionFailed => {
                    live_status.set(Some("failed".to_string()));
                }
                _ => {}
            }
        }
    });

    let live_steps_snap = live_steps.read().clone();
    let live_status_snap = live_status.read().clone();

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Execute Workflow",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                style: "min-width:600px; max-width:800px;",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Execute: {wf_name}" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    div { class: "form-group",
                        label { class: "form-label", "Input JSON" }
                        textarea {
                            class: if input_parse_err.read().is_some() { "form-textarea wf-field-error" } else { "form-textarea" },
                            style: "font-family: monospace; font-size: 12px; min-height: 100px;",
                            value: "{execution_input}",
                            oninput: move |e| {
                                let raw = e.value();
                                execution_input.set(raw.clone());
                                if raw.trim().is_empty() || raw.trim() == "{}" {
                                    input_parse_err.set(None);
                                } else {
                                    match serde_json::from_str::<serde_json::Value>(&raw) {
                                        Ok(_) => input_parse_err.set(None),
                                        Err(err) => input_parse_err.set(Some(format!("Invalid JSON: {err}"))),
                                    }
                                }
                            },
                        }
                        if let Some(err) = &*input_parse_err.read() {
                            div { class: "wf-field-error-msg", "{err}" }
                        }
                    }

                    // Live step progress (shown while executing or after events arrive)
                    if !live_steps_snap.is_empty() {
                        div { class: "wf-exec-results",
                            if let Some(ref status) = live_status_snap {
                                div { class: "wf-exec-summary",
                                    span {
                                        class: match status.as_str() {
                                            "success" => "badge-status badge-completed",
                                            "failed" => "badge-status badge-failed",
                                            _ => "badge-status badge-running",
                                        },
                                        "{status}"
                                    }
                                }
                            }
                            div { class: "wf-exec-timeline",
                                for ls in live_steps_snap.iter() {
                                    { render_live_step(ls) }
                                }
                            }
                        }
                    }

                    // Final API results (detailed output/errors per step)
                    if let Some(result) = &*execution_result.read() {
                        match result {
                            Ok(resp) => rsx! {
                                div { class: "wf-exec-results",
                                    div { class: "wf-exec-summary",
                                        span {
                                            class: if resp.status == "success" { "badge-status badge-completed" } else { "badge-status badge-failed" },
                                            "{resp.status}"
                                        }
                                        span { class: "text-secondary", style: "margin-left:8px;",
                                            "{resp.successful_steps} succeeded, {resp.failed_steps} failed \u{2014} {resp.total_duration_ms}ms"
                                        }
                                    }
                                    div { class: "wf-exec-timeline",
                                        for step_result in resp.steps.iter() {
                                            { render_step_result(step_result) }
                                        }
                                    }
                                }
                            },
                            Err(e) => rsx! {
                                div { class: "wf-validation-errors",
                                    div { class: "wf-validation-error", "{e}" }
                                }
                            },
                        }
                    }
                }
                div { class: "modal-footer",
                    button {
                        class: "btn-primary",
                        disabled: *executing.read() || input_parse_err.read().is_some(),
                        onclick: move |_| {
                            let input_str = execution_input.read().clone();
                            let input_json: serde_json::Value = serde_json::from_str(&input_str)
                                .unwrap_or_else(|_| json!({}));
                            executing.set(true);
                            execution_result.set(None);
                            live_steps.set(vec![]);
                            live_status.set(None);
                            exec_correlation.set(None);
                            spawn(async move {
                                let result = crate::api::execute_workflow(
                                    wf_id,
                                    ExecuteWorkflowRequest {
                                        input: input_json,
                                        correlation_id: None,
                                    },
                                ).await;
                                executing.set(false);
                                execution_result.set(Some(result));
                            });
                        },
                        if *executing.read() { "Executing\u{2026}" } else { "Execute" }
                    }
                    button {
                        class: "btn-secondary",
                        onclick: move |_| on_close.call(()),
                        "Close"
                    }
                }
            }
        }
    }
}

/// Render a single live step status row (from WebSocket events).
fn render_live_step(ls: &LiveStepStatus) -> Element {
    let status_class = match ls.status.as_str() {
        "Success" | "success" => "wf-step-success",
        "Failed" | "failed" => "wf-step-failed",
        "Skipped" | "skipped" => "wf-step-skipped",
        "running" => "wf-step-running",
        _ => "wf-step-pending",
    };
    let status_icon = match ls.status.as_str() {
        "Success" | "success" => "\u{2705}",
        "Failed" | "failed" => "\u{274C}",
        "Skipped" | "skipped" => "\u{23ED}",
        "running" => "\u{1F504}",
        _ => "\u{23F3}",
    };

    rsx! {
        div { class: "wf-exec-step {status_class}",
            div { class: "wf-exec-step-header",
                span { "{status_icon}" }
                span { class: "wf-exec-step-name", "{ls.step_id}" }
                span { class: "text-secondary", "({ls.skill_name})" }
                if ls.duration_ms > 0 {
                    span { class: "text-secondary", style: "margin-left:auto;", "{ls.duration_ms}ms" }
                }
            }
        }
    }
}

fn render_step_result(step: &StepResultDetail) -> Element {
    let status_class = match step.status.as_str() {
        "success" => "wf-step-success",
        "failed" => "wf-step-failed",
        "skipped" => "wf-step-skipped",
        _ => "wf-step-pending",
    };
    let status_icon = match step.status.as_str() {
        "success" => "\u{2705}",
        "failed" => "\u{274C}",
        "skipped" => "\u{23ED}",
        _ => "\u{23F3}",
    };
    let output_str = step.output.as_ref().map_or_else(String::new, |v| {
        serde_json::to_string_pretty(v).unwrap_or_default()
    });
    let error_str = step.error.clone().unwrap_or_default();

    rsx! {
        div { class: "wf-exec-step {status_class}",
            div { class: "wf-exec-step-header",
                span { "{status_icon}" }
                span { class: "wf-exec-step-name", "{step.step_id}" }
                span { class: "text-secondary", "({step.skill_name})" }
                span { class: "text-secondary", style: "margin-left:auto;", "{step.duration_ms}ms" }
            }
            if !error_str.is_empty() {
                div { class: "wf-exec-step-error", "{error_str}" }
            }
            if !output_str.is_empty() {
                pre { style: "font-size:11px; max-height:120px; overflow:auto;", "{output_str}" }
            }
        }
    }
}

// =============================================================================
// WORKFLOW HISTORY PANEL
// =============================================================================

/// Panel showing past workflow execution history.
/// Queries tasks with title starting with "Workflow execution:" and displays
/// them in a table with status, timestamps, and detail expansion.
#[component]
fn WorkflowHistoryPanel() -> Element {
    let store = use_context::<EventStreamStore>();
    let mut refresh = use_signal(|| 0_u64);
    let mut expanded_task = use_signal(|| Option::<Uuid>::None);
    let mut expanded_detail = use_signal(|| Option::<serde_json::Value>::None);

    // Fetch all tasks and filter to workflow execution records
    let history_resource = use_resource(move || async move {
        let _ = refresh();
        let tasks = crate::api::list_tasks()
            .await
            .map(|r| r.tasks)
            .unwrap_or_default();
        // Filter to workflow execution tasks
        tasks
            .into_iter()
            .filter(|t| t.title.starts_with("Workflow execution:"))
            .collect::<Vec<TaskDetail>>()
    });

    // Refresh on workflow execution events
    use_effect(move || {
        let events = store.events.read();
        if let Some(last) = events.back() {
            if matches!(
                &last.event_type,
                EventType::WorkflowExecutionCompleted | EventType::WorkflowExecutionFailed
            ) {
                refresh += 1;
            }
        }
    });

    let history_read = history_resource.read();
    let history: Vec<TaskDetail> = (*history_read)
        .as_ref()
        .map_or_else(Vec::new, std::clone::Clone::clone);

    let expanded_id = expanded_task.read().clone();
    let detail_snap = expanded_detail.read().clone();

    rsx! {
        div { class: "panel-scroll",
            h3 { style: "margin:0 0 12px 0; color:#E0E0E0;", "Workflow Execution History" }
            if history_read.is_none() {
                div { class: "state-message",
                    div { class: "spinner" }
                    span { "Loading history\u{2026}" }
                }
            } else if history.is_empty() {
                div { class: "state-message",
                    span { class: "state-icon", "\u{1F4CB}" }
                    span { "No workflow executions found yet." }
                }
            } else {
                table { class: "data-table",
                    thead {
                        tr {
                            th { "Workflow" }
                            th { "Status" }
                            th { "Created" }
                            th { "Updated" }
                            th { "Actions" }
                        }
                    }
                    tbody {
                        for task in history.iter() {
                            { render_history_row(task, &expanded_id, &mut expanded_task, &mut expanded_detail) }
                        }
                    }
                }
                // Expanded detail view
                if let (Some(_eid), Some(ref detail)) = (expanded_id, detail_snap) {
                    div { class: "wf-exec-results", style: "margin-top:12px;",
                        if let Some(steps) = detail.get("steps").and_then(|v| v.as_array()) {
                            div { class: "wf-exec-timeline",
                                for step_json in steps.iter() {
                                    { render_history_step(step_json) }
                                }
                            }
                        }
                        if let Some(summary) = detail.get("execution_summary").and_then(|v| v.as_str()) {
                            div { class: "text-secondary", style: "margin-top:8px; font-size:12px;", "{summary}" }
                        }
                    }
                }
            }
        }
    }
}

fn render_history_row(
    task: &TaskDetail,
    expanded_id: &Option<Uuid>,
    expanded_task: &Signal<Option<Uuid>>,
    expanded_detail: &Signal<Option<serde_json::Value>>,
) -> Element {
    let wf_name = task
        .title
        .strip_prefix("Workflow execution: ")
        .unwrap_or(&task.title)
        .to_string();
    let status_badge = match task.state.as_str() {
        "completed" => "badge-status badge-completed",
        "failed" => "badge-status badge-failed",
        _ => "badge-status badge-running",
    };
    let created = task.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
    let updated = task.updated_at.format("%Y-%m-%d %H:%M:%S").to_string();
    let task_id = task.task_id;
    let is_expanded = *expanded_id == Some(task_id);
    let mut expanded_task = *expanded_task;
    let mut expanded_detail = *expanded_detail;

    rsx! {
        tr {
            td { "{wf_name}" }
            td { span { class: "{status_badge}", "{task.state}" } }
            td { "{created}" }
            td { "{updated}" }
            td {
                button {
                    class: if is_expanded { "btn-primary btn-sm" } else { "btn-secondary btn-sm" },
                    onclick: move |_| {
                        if is_expanded {
                            expanded_task.set(None);
                            expanded_detail.set(None);
                        } else {
                            expanded_task.set(Some(task_id));
                            // Fetch run details for this task
                            spawn(async move {
                                let runs = crate::api::list_task_runs(task_id).await.unwrap_or_default();
                                // The result JSON is stored in the first run's result field
                                if let Some(first_run) = runs.first() {
                                    if let Some(ref result) = first_run.result {
                                        expanded_detail.set(Some(result.clone()));
                                    }
                                }
                            });
                        }
                    },
                    if is_expanded { "Hide" } else { "Details" }
                }
            }
        }
    }
}

fn render_history_step(step_json: &serde_json::Value) -> Element {
    let step_id = step_json
        .get("step_id")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let skill_name = step_json
        .get("skill_name")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let status = step_json
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let duration_ms = step_json
        .get("duration_ms")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let error = step_json
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let status_class = match status {
        "Success" | "success" => "wf-step-success",
        "Failed" | "failed" => "wf-step-failed",
        "Skipped" | "skipped" => "wf-step-skipped",
        _ => "wf-step-pending",
    };
    let status_icon = match status {
        "Success" | "success" => "\u{2705}",
        "Failed" | "failed" => "\u{274C}",
        "Skipped" | "skipped" => "\u{23ED}",
        _ => "\u{23F3}",
    };

    rsx! {
        div { class: "wf-exec-step {status_class}",
            div { class: "wf-exec-step-header",
                span { "{status_icon}" }
                span { class: "wf-exec-step-name", "{step_id}" }
                span { class: "text-secondary", "({skill_name})" }
                span { class: "text-secondary", style: "margin-left:auto;", "{duration_ms}ms" }
            }
            if !error.is_empty() {
                div { class: "wf-exec-step-error", "{error}" }
            }
        }
    }
}

// =============================================================================
// CREATE WORKFLOW MODAL
// =============================================================================

#[component]
fn CreateWorkflowModal(on_close: EventHandler, on_created: EventHandler) -> Element {
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut saving = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Create Workflow",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Create Workflow" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    if let Some(err) = &*error_msg.read() {
                        div { class: "wf-validation-errors",
                            div { class: "wf-validation-error", "{err}" }
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Name" }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "Workflow name",
                            value: "{name}",
                            oninput: move |e| name.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { class: "form-label", "Description" }
                        textarea {
                            class: "form-textarea",
                            placeholder: "Optional description",
                            value: "{description}",
                            oninput: move |e| description.set(e.value()),
                        }
                    }
                }
                div { class: "modal-footer",
                    button {
                        class: "btn-primary",
                        disabled: *saving.read(),
                        onclick: move |_| {
                            let n = name.read().trim().to_string();
                            if n.is_empty() {
                                error_msg.set(Some("Name is required".into()));
                                return;
                            }
                            error_msg.set(None);
                            saving.set(true);
                            let desc = description.read().trim().to_string();
                            spawn(async move {
                                match crate::api::create_workflow(CreateWorkflowRequest {
                                    name: n,
                                    description: if desc.is_empty() { None } else { Some(desc) },
                                    steps: vec![],
                                }).await {
                                    Ok(_) => on_created.call(()),
                                    Err(e) => {
                                        saving.set(false);
                                        error_msg.set(Some(e));
                                    }
                                }
                            });
                        },
                        if *saving.read() { "Creating\u{2026}" } else { "Create" }
                    }
                    button {
                        class: "btn-secondary",
                        onclick: move |_| on_close.call(()),
                        "Cancel"
                    }
                }
            }
        }
    }
}

// =============================================================================
// DELETE CONFIRM MODAL
// =============================================================================

#[component]
fn DeleteConfirmModal(
    workflow_id: Uuid,
    on_close: EventHandler,
    on_deleted: EventHandler,
) -> Element {
    let mut deleting = use_signal(|| false);

    rsx! {
        div {
            class: "modal-overlay",
            role: "dialog",
            aria_label: "Delete Workflow",
            onclick: move |_| on_close.call(()),
            div {
                class: "modal-content",
                style: "min-width:400px; max-width:480px;",
                onclick: move |e| e.stop_propagation(),
                div { class: "modal-header",
                    h2 { "Confirm Delete" }
                    button {
                        class: "btn-icon",
                        onclick: move |_| on_close.call(()),
                        "\u{2715}"
                    }
                }
                div { class: "modal-body",
                    p { "Are you sure you want to delete this workflow? This action cannot be undone." }
                }
                div { class: "modal-footer",
                    button {
                        class: "btn-danger",
                        disabled: *deleting.read(),
                        onclick: move |_| {
                            deleting.set(true);
                            spawn(async move {
                                match crate::api::delete_workflow(workflow_id).await {
                                    Ok(()) => on_deleted.call(()),
                                    Err(e) => {
                                        tracing::warn!(error = %e, "Failed to delete workflow");
                                        deleting.set(false);
                                    }
                                }
                            });
                        },
                        if *deleting.read() { "Deleting\u{2026}" } else { "Delete" }
                    }
                    button {
                        class: "btn-secondary",
                        onclick: move |_| on_close.call(()),
                        "Cancel"
                    }
                }
            }
        }
    }
}
