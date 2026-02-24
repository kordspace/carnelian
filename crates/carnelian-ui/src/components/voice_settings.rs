//! Voice settings component — configure ElevenLabs API key, select voice, test TTS.

use carnelian_common::types::{ConfigureVoiceRequest, TestVoiceRequest};
use dioxus::document::eval;
use dioxus::prelude::*;

use crate::store::EventStreamStore;

/// Voice settings panel with API key configuration, voice selection, and TTS test.
#[component]
pub fn VoiceSettings() -> Element {
    let store = use_context::<EventStreamStore>();
    let xp = store.xp_state.read();
    let identity_id = xp.identity_id;

    let mut api_key = use_signal(String::new);
    let mut selected_voice_id = use_signal(String::new);
    let mut test_text = use_signal(|| "Hello, I am Carnelian.".to_string());
    let mut status_message = use_signal(|| Option::<String>::None);
    let mut saving = use_signal(|| false);
    let mut testing = use_signal(|| false);

    let eval = eval;

    let voices = use_resource(|| async {
        crate::api::list_voices().await.ok()
    });

    let voices_read = voices.read();
    let voice_list = (*voices_read).as_ref().and_then(|o| o.as_ref());

    rsx! {
        div { class: "voice-settings-panel",
            h2 { style: "margin-bottom: 16px;", "Voice Settings" }

            // API Key
            div { class: "form-group",
                label { class: "form-label", "ElevenLabs API Key" }
                input {
                    class: "form-input",
                    r#type: "password",
                    placeholder: "Enter API key...",
                    value: "{api_key}",
                    oninput: move |e| api_key.set(e.value()),
                }
            }

            // Voice selection
            div { class: "form-group",
                label { class: "form-label", "Default Voice" }
                select {
                    class: "filter-select",
                    value: "{selected_voice_id}",
                    onchange: move |e| selected_voice_id.set(e.value()),
                    if voice_list.is_none() {
                        option { disabled: true, "Loading voices\u{2026}" }
                    }
                    if let Some(resp) = voice_list {
                        option { value: "", "Select a voice..." }
                        for voice in &resp.voices {
                            option { value: "{voice.voice_id}", "{voice.name}" }
                        }
                    }
                }
            }

            // Save button
            div { style: "display: flex; gap: 8px; margin-bottom: 16px;",
                button {
                    class: "btn-primary",
                    disabled: saving(),
                    onclick: move |_| {
                        let key = api_key.read().clone();
                        let voice_id = selected_voice_id.read().clone();
                        saving.set(true);
                        status_message.set(None);
                        spawn(async move {
                            let req = ConfigureVoiceRequest {
                                api_key: key,
                                default_voice_id: if voice_id.is_empty() { None } else { Some(voice_id) },
                                identity_id,
                            };
                            match crate::api::configure_voice(req).await {
                                Ok(_) => status_message.set(Some("Voice configured successfully".to_string())),
                                Err(e) => status_message.set(Some(format!("Error: {e}"))),
                            }
                            saving.set(false);
                        });
                    },
                    if saving() { "Saving..." } else { "Save Configuration" }
                }
            }

            // Status message
            {
                let msg_opt = status_message.read().clone();
                if let Some(ref msg) = msg_opt {
                    let color = if msg.starts_with("Error") { "#E74C3C" } else { "#2ECC71" };
                    let style = format!("font-size: 13px; margin-bottom: 16px; color: {color}");
                    rsx! {
                        div { style: "{style}", "{msg}" }
                    }
                } else {
                    rsx! {}
                }
            }

            // Test TTS
            div { class: "form-group",
                label { class: "form-label", "Test Text-to-Speech" }
                input {
                    class: "form-input",
                    placeholder: "Enter text to speak...",
                    value: "{test_text}",
                    oninput: move |e| test_text.set(e.value()),
                }
            }

            button {
                class: "btn-secondary",
                disabled: testing() || selected_voice_id.read().is_empty(),
                onclick: move |_| {
                    let text = test_text.read().clone();
                    let voice_id = selected_voice_id.read().clone();
                    testing.set(true);
                    status_message.set(None);
                    spawn(async move {
                        let req = TestVoiceRequest { text, voice_id };
                        match crate::api::test_voice(req).await {
                            Ok(resp) => {
                                let data_url = format!("data:{};base64,{}", resp.content_type, resp.audio_base64);
                                let js = format!("(function(){{const a=new Audio('{}');a.play();}})()", data_url);
                                eval(&js);
                                status_message.set(Some("Playing audio...".to_string()));
                            }
                            Err(e) => status_message.set(Some(format!("Error: {e}"))),
                        }
                        testing.set(false);
                    });
                },
                if testing() { "Testing..." } else { "Test Voice" }
            }
        }
    }
}
