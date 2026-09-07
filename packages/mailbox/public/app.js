const $ = (id) => document.getElementById(id);
let selected;
let loading = false;

async function api(path, options) {
  const response = await fetch(path, options);
  if (!response.ok) throw new Error(`请求失败 (${response.status})`);
  return response.json();
}

function status(message) {
  $("status").textContent = message;
}

function reset() {
  selected = undefined;
  $("detail").hidden = true;
  $("empty").hidden = false;
  $("html").removeAttribute("src");
}

async function select(id) {
  selected = id;
  try {
    const mail = await api(`/api/messages/${id}`);
    if (selected !== id) return;
    $("empty").hidden = true;
    $("detail").hidden = false;
    $("subject").textContent = mail.subject || "(无主题)";
    $("metadata").replaceChildren();
    for (const [label, value] of [
      ["发件人", mail.envelope.from],
      ["收件人", mail.envelope.to.join(", ")],
      ["接收时间", new Date(mail.receivedAt).toLocaleString()],
      ["Message-ID", mail.messageId ?? "—"],
    ]) {
      const dt = document.createElement("dt");
      const dd = document.createElement("dd");
      dt.textContent = label;
      dd.textContent = value;
      $("metadata").append(dt, dd);
    }
    $("text").textContent = mail.text || "(无纯文本内容)";
    $("headers").textContent = mail.headerLines.map(({ line }) => line).join("\n");
    $("html").src = `/api/messages/${id}/preview`;
    $("download").href = `/api/messages/${id}/raw`;
    document
      .querySelectorAll(".mail")
      .forEach((button) => button.classList.toggle("selected", button.dataset.id === id));
  } catch (error) {
    if (selected === id) reset();
    status(error.message);
  }
}

async function refresh() {
  if (loading) return;
  loading = true;
  try {
    const query = new URLSearchParams();
    if ($("recipient").value.trim()) query.set("recipient", $("recipient").value.trim());
    const result = await api(`/api/messages?${query}`);
    $("count").textContent = `${result.total} 封邮件`;
    $("messages").replaceChildren();
    for (const mail of result.messages) {
      const button = document.createElement("button");
      button.className = "mail";
      button.dataset.id = mail.id;
      button.classList.toggle("selected", mail.id === selected);
      for (const [tag, value] of [
        ["strong", mail.subject || "(无主题)"],
        ["span", mail.envelope.to.join(", ")],
        ["span", new Date(mail.receivedAt).toLocaleString()],
      ]) {
        const node = document.createElement(tag);
        node.textContent = value;
        button.append(node);
      }
      button.onclick = () => {
        void select(mail.id);
      };
      $("messages").append(button);
    }
    if (selected && !result.messages.some((mail) => mail.id === selected)) reset();
    status(`已更新 · ${(result.storedBytes / 1024).toFixed(1)} KiB 原文`);
  } catch (error) {
    status(error.message);
  } finally {
    loading = false;
  }
}

$("filter").onsubmit = (event) => {
  event.preventDefault();
  void refresh();
};
$("refresh").onclick = () => {
  void refresh();
};
$("clear").onclick = async () => {
  try {
    await api("/api/messages", { method: "DELETE" });
    reset();
    await refresh();
  } catch (error) {
    status(error.message);
  }
};
$("delete").onclick = async () => {
  if (!selected) return;
  try {
    await api(`/api/messages/${selected}`, { method: "DELETE" });
    reset();
    await refresh();
  } catch (error) {
    status(error.message);
  }
};
document.querySelectorAll("[data-tab]").forEach((button) => {
  button.onclick = () => {
    for (const id of ["text", "html", "headers"]) $(id).hidden = id !== button.dataset.tab;
    document
      .querySelectorAll("[data-tab]")
      .forEach((tab) => tab.classList.toggle("active", tab === button));
  };
});
void refresh();
setInterval(() => {
  void refresh();
}, 2000);
