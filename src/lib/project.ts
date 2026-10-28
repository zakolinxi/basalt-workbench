import { open } from "@tauri-apps/plugin-dialog";
import { api, errorMessage } from "./api";
import { useAppStore } from "./store";

export async function pickAndAddProject(): Promise<void> {
  const notify = useAppStore.getState().notify;
  try {
    const selection = await open({
      directory: true,
      multiple: false,
      title: "选择项目目录",
    });
    const path = Array.isArray(selection) ? selection[0] : selection;
    if (!path) return;
    const project = await api.projectAdd(path);
    await useAppStore.getState().addProject(project.id);
    notify("success", "项目已添加", project.path);
  } catch (error) {
    notify("error", "添加项目失败", errorMessage(error));
  }
}
