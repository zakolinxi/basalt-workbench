import { api, errorMessage } from "./api";
import { isRemote } from "./bridge";
import { pickPaths } from "./platform";
import { useAppStore } from "./store";
import { requestDirectory } from "../components/DirectoryPicker";

export async function pickAndAddProject(): Promise<void> {
  const notify = useAppStore.getState().notify;
  try {
    // A phone cannot see the PC's disks, so it browses the desktop's file
    // system through the gateway instead of a native dialog.
    const path = isRemote()
      ? await requestDirectory("选择项目目录")
      : (
          await pickPaths({
            directory: true,
            multiple: false,
            title: "选择项目目录",
          })
        )[0];
    if (!path) return;
    const project = await api.projectAdd(path);
    await useAppStore.getState().addProject(project.id);
    notify("success", "项目已添加", project.path);
  } catch (error) {
    notify("error", "添加项目失败", errorMessage(error));
  }
}
