const state = document.getElementById("state");
const output = document.getElementById("status");

async function load() {
  try {
    const status = await window.__TAURI__.core.invoke("status");
    state.textContent = `${status.repo} @ ${status.git_ref}`;
    output.textContent = JSON.stringify(status, null, 2);
  } catch (error) {
    state.textContent = `Erro: ${error}`;
  }
}

load();
