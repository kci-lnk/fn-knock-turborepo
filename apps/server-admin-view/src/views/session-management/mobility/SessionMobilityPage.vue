<template>
  <div class="space-y-6">
    <Breadcrumb>
      <BreadcrumbList>
        <BreadcrumbItem>
          <BreadcrumbLink href="#/sessions?tab=sessions">
            {{ t("admin.nav.sessions") }}
          </BreadcrumbLink>
        </BreadcrumbItem>
        <BreadcrumbSeparator />
        <BreadcrumbItem>
          <BreadcrumbPage>{{
            t("admin.sessions.mobilityPage.title")
          }}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>

    <Card class="border-border/50 bg-background shadow-none">
      <CardHeader class="gap-0">
        <div class="min-w-0 space-y-3">
          <div class="flex items-start justify-between gap-3">
            <div class="flex min-w-0 flex-1 flex-wrap items-center gap-2">
              <CardTitle
                class="break-words text-xl font-semibold tracking-[0.02em] sm:text-[1.65rem]"
              >
                {{ t("admin.sessions.mobilityPage.title") }}
              </CardTitle>
              <Badge
                v-if="session"
                variant="secondary"
                class="rounded-full border border-border/40 bg-muted/30 px-2.5 py-0.5 text-muted-foreground shadow-none"
              >
                {{ session.method }}
              </Badge>
            </div>

            <Button
              variant="outline"
              size="icon"
              class="mt-0.5 h-9 w-9 shrink-0 rounded-full border-border/40 bg-background text-muted-foreground transition-colors hover:bg-muted/30 hover:text-foreground"
              :aria-label="sortToggleLabel"
              :title="sortToggleLabel"
              @click="toggleSortOrder"
            >
              <ArrowUpDown
                class="h-4 w-4 transition-transform duration-200"
                :class="sortOrder === 'desc' ? 'rotate-180' : ''"
              />
              <span class="sr-only">{{ sortToggleLabel }}</span>
            </Button>
          </div>

          <CardDescription
            class="max-w-2xl break-all text-sm leading-7 text-muted-foreground/90"
          >
            {{ headerDescription }}
          </CardDescription>
        </div>
      </CardHeader>

      <CardContent class="space-y-7 pt-0 px-5">
        <div
          v-if="isLoading"
          class="flex items-center justify-center py-16 text-sm text-muted-foreground"
          role="status"
        >
          <span
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent"
          ></span>
          {{ t("admin.sessions.mobilityPage.loading") }}
        </div>

        <div
          v-else-if="loadError"
          class="rounded-xl border border-destructive/30 bg-destructive/5 px-4 py-5"
          role="alert"
        >
          <div class="text-sm font-medium text-destructive">
            {{ t("admin.sessions.mobilityPage.loadFailed") }}
          </div>
          <div class="mt-1 text-sm text-muted-foreground">{{ loadError }}</div>
        </div>

        <template v-else-if="session">
          <div
            class="grid gap-3 sm:grid-cols-2 xl:grid-cols-[minmax(0,1.2fr)_repeat(3,minmax(0,1fr))]"
          >
            <div
              class="rounded-2xl border border-border/35 bg-muted/[0.14] px-5 py-4 sm:col-span-2 xl:col-span-1"
            >
              <div
                class="text-[11px] font-medium uppercase tracking-[0.18em] text-muted-foreground/90"
              >
                {{ t("admin.sessions.mobilityPage.currentSession") }}
              </div>
              <div class="mt-3 break-words text-sm font-medium text-foreground">
                {{ session.credentialName }}
              </div>
              <div class="mt-3 break-all font-mono text-sm text-foreground">
                {{ session.ip }}
              </div>
              <div
                class="mt-1 break-words text-xs leading-6 text-muted-foreground"
              >
                {{
                  session.ipLocation ||
                  t("admin.sessions.mobilityPage.noLocation")
                }}
              </div>
            </div>

            <div
              class="rounded-2xl border border-border/35 bg-muted/[0.14] px-5 py-4"
            >
              <div
                class="text-[11px] font-medium uppercase tracking-[0.18em] text-muted-foreground/90"
              >
                {{ t("admin.sessions.mobilityPage.recoveryCount") }}
              </div>
              <div class="mt-3 text-xl font-semibold text-foreground">
                {{ mobilitySummary?.driftCount ?? 0 }}
              </div>
              <div
                class="mt-1 break-words text-xs leading-6 text-muted-foreground"
              >
                {{ driftCountDescription }}
              </div>
            </div>

            <div
              class="rounded-2xl border border-border/35 bg-muted/[0.14] px-5 py-4"
            >
              <div
                class="text-[11px] font-medium uppercase tracking-[0.18em] text-muted-foreground/90"
              >
                {{ t("admin.sessions.mobilityPage.timelineSpan") }}
              </div>
              <div
                class="mt-3 break-words text-sm font-semibold text-foreground"
              >
                {{ timelineSpanLabel }}
              </div>
              <div
                class="mt-1 break-words text-xs leading-6 text-muted-foreground"
              >
                {{ timelineSpanDescription }}
              </div>
            </div>

            <div
              class="rounded-2xl border border-border/35 bg-muted/[0.14] px-5 py-4"
            >
              <div
                class="text-[11px] font-medium uppercase tracking-[0.18em] text-muted-foreground/90"
              >
                {{ t("admin.sessions.mobilityPage.latestChange") }}
              </div>
              <div
                class="mt-3 break-words text-sm font-semibold text-foreground"
              >
                <HumanFriendlyTime
                  v-if="lastEventTimeValue"
                  :value="lastEventTimeValue"
                  :locale="locale"
                />
                <template v-else>{{ lastEventTimeLabel }}</template>
              </div>
              <div
                class="mt-1 break-words text-xs leading-6 text-muted-foreground"
              >
                {{ lastEventSourceLabel }}
              </div>
            </div>
          </div>

          <div
            v-if="timelineEntries.length > 0"
            class="relative border-t border-border/35 pt-6 sm:pt-8"
          >
            <div
              class="absolute bottom-4 left-6 top-8 w-px bg-border/70 sm:left-8 sm:top-10"
            />
            <div class="space-y-0">
              <div
                v-for="entry in timelineEntries"
                :key="entry.id"
                class="relative border-b border-border/40 py-5 pl-11 first:pt-0 last:border-b-0 last:pb-0 sm:py-6 sm:pl-14"
              >
                <div
                  class="pointer-events-none absolute left-6 top-5 -translate-x-1/2 sm:left-8"
                >
                  <LiveStatusBadge
                    v-if="entry.id === latestEntryId"
                    active
                    :pulse="false"
                    :active-label="
                      t('admin.sessions.mobilityPage.latestStatus')
                    "
                    size="sm"
                    class="block"
                  />
                  <div
                    v-else
                    class="h-3 w-3 rounded-full border-2 border-background bg-foreground/90"
                  />
                </div>
                <div class="space-y-4">
                  <div
                    class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between"
                  >
                    <div class="min-w-0 space-y-1">
                      <div class="flex flex-wrap items-center gap-2">
                        <Badge
                          :variant="
                            entry.event.kind === 'login'
                              ? 'secondary'
                              : 'outline'
                          "
                          class="rounded-full border-border/40 bg-background/80 px-2.5 py-0.5 text-[12px] font-medium shadow-none"
                        >
                          {{
                            entry.event.kind === "login"
                              ? t(
                                  "admin.sessions.mobilityPage.loginEstablished",
                                )
                              : t("admin.sessions.mobilityPage.ipRecovered")
                          }}
                        </Badge>
                        <span
                          class="break-words text-sm font-medium leading-6 text-foreground"
                          >{{ entry.title }}</span
                        >
                      </div>
                      <div
                        class="break-words text-xs leading-6 text-muted-foreground"
                      >
                        {{ entry.subtitle }}
                      </div>
                    </div>
                    <div
                      class="text-left text-xs leading-6 text-muted-foreground sm:shrink-0 sm:pl-6 sm:text-right"
                    >
                      <div>
                        <HumanFriendlyTime
                          :value="entry.event.happenedAt"
                          :locale="locale"
                        />
                      </div>
                      <div v-if="entry.gapLabel" class="mt-1">
                        {{ entry.gapLabel }}
                      </div>
                    </div>
                  </div>

                  <div
                    v-if="entry.event.kind === 'login'"
                    class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-foreground"
                  >
                    <span class="break-all font-mono">{{
                      entry.event.toIp
                    }}</span>
                    <span class="text-muted-foreground/70">·</span>
                    <span class="break-words text-muted-foreground">{{
                      entry.event.toIpLocation ||
                      t("admin.sessions.mobilityPage.noLocation")
                    }}</span>
                  </div>

                  <div
                    v-else
                    class="mt-1 grid gap-3 text-sm sm:grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] sm:items-center"
                  >
                    <div class="min-w-0">
                      <div
                        class="text-[11px] uppercase tracking-[0.16em] text-muted-foreground/85"
                      >
                        {{ t("admin.sessions.mobilityPage.beforeDrift") }}
                      </div>
                      <div class="mt-2 break-all font-mono text-foreground">
                        {{ entry.event.fromIp }}
                      </div>
                      <div
                        class="mt-1 break-words text-xs leading-6 text-muted-foreground"
                      >
                        {{
                          entry.event.fromIpLocation ||
                          t("admin.sessions.mobilityPage.noLocation")
                        }}
                      </div>
                    </div>
                    <div class="flex justify-center text-muted-foreground/70">
                      <ArrowRight class="h-4 w-4 rotate-90 sm:rotate-0" />
                    </div>
                    <div class="min-w-0">
                      <div
                        class="text-[11px] uppercase tracking-[0.16em] text-muted-foreground/85"
                      >
                        {{ t("admin.sessions.mobilityPage.afterDrift") }}
                      </div>
                      <div class="mt-2 break-all font-mono text-foreground">
                        {{ entry.event.toIp }}
                      </div>
                      <div
                        class="mt-1 break-words text-xs leading-6 text-muted-foreground"
                      >
                        {{
                          entry.event.toIpLocation ||
                          t("admin.sessions.mobilityPage.noLocation")
                        }}
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div
            v-else
            class="border-t border-border/35 px-4 py-12 text-center text-sm text-muted-foreground"
          >
            {{ t("admin.sessions.mobilityPage.empty") }}
          </div>
        </template>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ArrowRight, ArrowUpDown } from "lucide-vue-next";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import LiveStatusBadge from "@/components/LiveStatusBadge.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import { useSessionMobilityPage } from "./useSessionMobilityPage";

const {
  driftCountDescription,
  headerDescription,
  isLoading,
  lastEventSourceLabel,
  lastEventTimeLabel,
  lastEventTimeValue,
  latestEntryId,
  loadError,
  locale,
  mobilitySummary,
  session,
  sortOrder,
  sortToggleLabel,
  t,
  timelineEntries,
  timelineSpanDescription,
  timelineSpanLabel,
  toggleSortOrder,
} = useSessionMobilityPage();
</script>
