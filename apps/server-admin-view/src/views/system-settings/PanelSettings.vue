<script setup lang="ts">
import { computed, ref } from "vue";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ShieldAlert, ShieldCheck } from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { ConfigAPI } from "../../lib/api";
import { dockerAdminPanelResetCommands } from "../../lib/docker-admin-panel-reset";
import { useConfigStore } from "../../store/config";
import { useDockerAdminAuthStore } from "../../store/dockerAdminAuth";

const configStore = useConfigStore();
const dockerAdminAuthStore = useDockerAdminAuthStore();

const newPassword = ref("");
const confirmPassword = ref("");

const resetCommands = dockerAdminPanelResetCommands;

const isDockerMode = computed(() => configStore.isDockerDeployment);
const isFormFilled = computed(
  () =>
    newPassword.value.trim().length > 0 &&
    confirmPassword.value.trim().length > 0,
);

const { isPending: isSaving, run: runSavePassword } = useAsyncAction({
  onError: (error) => {
    toast.error("修改失败", {
      description: extractErrorMessage(error, "无法更新管理面板密码"),
    });
  },
});

const resetForm = () => {
  newPassword.value = "";
  confirmPassword.value = "";
};

const savePassword = async () => {
  const password = newPassword.value.trim();
  const confirm = confirmPassword.value.trim();

  if (!password) {
    toast.error("请先输入新密码");
    return;
  }
  if (password !== confirm) {
    toast.error("两次输入的新密码不一致");
    return;
  }

  const changed = await runSavePassword(async () => {
    await ConfigAPI.changeDockerAdminPassword(password);
    await dockerAdminAuthStore.bootstrap({ force: true });
    return true;
  });
  if (!changed) return;

  resetForm();
  toast.success("管理面板密码已更新", {
    description: "当前会话已刷新，其他已登录的管理面板会话会失效。",
  });
};
</script>

<template>
  <div v-if="isDockerMode" class="space-y-4">
    <Card>
      <CardHeader>
        <CardTitle>管理面板密码</CardTitle>
        <CardDescription>
          这里修改的是 Docker 管理入口的面板登录密码，不影响业务侧的鉴权配置。
          修改后会立即轮换当前管理会话，并使其他已登录的面板会话失效。
        </CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="space-y-2">
          <Label for="docker-panel-password">新密码</Label>
          <Input
            id="docker-panel-password"
            v-model="newPassword"
            type="password"
            autocomplete="new-password"
            placeholder="请输入新的管理面板密码"
            :disabled="isSaving"
          />
        </div>

        <div class="space-y-2">
          <Label for="docker-panel-password-confirm">确认新密码</Label>
          <Input
            id="docker-panel-password-confirm"
            v-model="confirmPassword"
            type="password"
            autocomplete="new-password"
            placeholder="请再次输入新的管理面板密码"
            :disabled="isSaving"
            @keyup.enter="savePassword"
          />
        </div>

        <Alert>
          <ShieldCheck class="h-4 w-4" />
          <AlertTitle>密码规则</AlertTitle>
          <AlertDescription>
            至少 6 位，且必须同时包含字母和数字，不能带空白字符。
          </AlertDescription>
        </Alert>

        <div class="flex items-center gap-3">
          <Button :disabled="isSaving || !isFormFilled" @click="savePassword">
            <span
              v-if="isSaving"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            修改密码
          </Button>
          <Button variant="outline" :disabled="isSaving" @click="resetForm">
            清空
          </Button>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardHeader>
        <CardTitle>忘记密码时</CardTitle>
        <CardDescription>
          如果已经无法登录管理面板，可以在容器外直接执行重置命令。该操作只会清除面板密码、面板会话和登录退避状态，不会删除业务配置。
        </CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <Alert>
          <ShieldAlert class="h-4 w-4" />
          <AlertTitle>执行结果</AlertTitle>
          <AlertDescription>
            清理完成后，下次访问 Docker 管理入口会重新进入“首次设置密码”流程。
          </AlertDescription>
        </Alert>

        <div class="space-y-2">
          <p class="text-sm font-medium">1. 先登录 Docker 主机</p>
          <pre
            class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
          ><code>{{ resetCommands.ssh }}</code></pre>
        </div>

        <div class="space-y-2">
          <p class="text-sm font-medium">2. 推荐：在 compose 部署目录执行</p>
          <pre
            class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
          ><code>{{ resetCommands.compose }}</code></pre>
        </div>

        <div class="space-y-2">
          <p class="text-sm font-medium">
            3. 如果只知道容器在跑 Docker，可直接执行
          </p>
          <pre
            class="w-full max-w-full overflow-x-auto whitespace-pre-wrap break-words rounded-lg border bg-muted/40 px-3 py-3 text-sm leading-6"
          ><code>{{ resetCommands.dockerExec }}</code></pre>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
