import type { Dispatch, FormEvent, ReactNode, SetStateAction } from "react";
import { FolderOpen, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/components/ui/utils";
import {
  DRIVER_REGISTRY,
  getDefaultPort,
  supportsSSLCA,
} from "@/lib/driver-registry";
import {
  getConnectionFormCapabilities,
  isFileBasedDriver,
  requiresPasswordOnCreate,
  requiresUsername,
} from "@/lib/connection-form/rules";
import type { ConnectionForm, Driver } from "@/services/api";

interface ConnectionDialogTestMessage {
  ok: boolean;
  text: string;
  latency?: number;
}

interface ConnectionDialogProps {
  open: boolean;
  trigger: ReactNode;
  dialogMode: "create" | "edit";
  createStep: "type" | "details";
  form: ConnectionForm;
  setForm: Dispatch<SetStateAction<ConnectionForm>>;
  validationMsg: string | null;
  testMsg: ConnectionDialogTestMessage | null;
  requiredOk: boolean;
  isTesting: boolean;
  isConnecting: boolean;
  isSavingEdit: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (e: FormEvent<HTMLFormElement>) => void;
  onClose: () => void;
  onTestConnection: () => void;
  onCreateDriverSelect: (driver: Driver) => void;
  onBackToType: () => void;
  onPickSslCaCertFile: () => void;
  onPickSshKeyFile: () => void;
  onPickDatabaseFile: (driver: Driver) => void;
}

const isPreviewDriver = (driver: Driver) =>
  DRIVER_REGISTRY.find((item) => item.id === driver)?.importCapability ===
  "unsupported";

export function ConnectionDialog({
  open,
  trigger,
  dialogMode,
  createStep,
  form,
  setForm,
  validationMsg,
  testMsg,
  requiredOk,
  isTesting,
  isConnecting,
  isSavingEdit,
  onOpenChange,
  onSubmit,
  onClose,
  onTestConnection,
  onCreateDriverSelect,
  onBackToType,
  onPickSslCaCertFile,
  onPickSshKeyFile,
  onPickDatabaseFile,
}: ConnectionDialogProps) {
  const { t } = useTranslation();
  const driverConfig =
    DRIVER_REGISTRY.find((driver) => driver.id === form.driver) ??
    DRIVER_REGISTRY[0];
  const formCapabilities = getConnectionFormCapabilities(form.driver);
  const isFileBased = isFileBasedDriver(form.driver);
  const supportsSslCa = supportsSSLCA(form.driver);
  const isPasswordRequiredOnCreate = requiresPasswordOnCreate(form.driver);
  const isUsernameRequired = requiresUsername(form.driver);
  const isCreateTypeStep = dialogMode === "create" && createStep === "type";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>{trigger}</DialogTrigger>
      <DialogContent className="sm:max-w-2xl">
        <form onSubmit={onSubmit}>
          <DialogHeader>
            <DialogTitle>
              {dialogMode === "edit"
                ? t("connection.dialog.editTitle")
                : t("connection.dialog.newTitle")}
            </DialogTitle>
            <DialogDescription>
              {isCreateTypeStep
                ? t("connection.dialog.typeStepDescription")
                : t("connection.dialog.detailsStepDescription")}
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            {isCreateTypeStep ? (
              <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
                {DRIVER_REGISTRY.map((driver) => (
                  <button
                    key={driver.id}
                    type="button"
                    className="text-left"
                    onClick={() => onCreateDriverSelect(driver.id)}
                  >
                    <Card
                      className={cn(
                        "relative h-full transition-colors hover:border-primary/50 hover:bg-accent/30",
                        form.driver === driver.id &&
                          "border-primary bg-accent/20",
                      )}
                    >
                      <CardContent className="flex h-full flex-col gap-3 p-4">
                        {isPreviewDriver(driver.id) ? (
                          <Badge
                            variant="outline"
                            className="absolute top-3 right-3 font-normal"
                          >
                            {t("connection.dialog.driverHints.preview")}
                          </Badge>
                        ) : null}
                        <div className="flex h-full flex-col items-center justify-center gap-3 py-1 text-center">
                          <div className="flex h-16 w-16 items-center justify-center rounded-2xl border bg-muted/40 [&_svg]:h-8 [&_svg]:w-8">
                            {driver.icon()}
                          </div>
                          <div className="text-base font-medium">
                            {driver.label}
                          </div>
                        </div>
                      </CardContent>
                    </Card>
                  </button>
                ))}
              </div>
            ) : (
              <>
                <div className="flex items-center justify-between gap-3 rounded-lg border bg-muted/20 px-4 py-3">
                  <div className="flex items-center gap-3">
                    <div className="flex h-10 w-10 items-center justify-center rounded-lg border bg-background">
                      {driverConfig.icon()}
                    </div>
                    <div className="font-medium">{driverConfig.label}</div>
                  </div>
                  {dialogMode === "create" ? (
                    <Button
                      type="button"
                      variant="ghost"
                      onClick={onBackToType}
                    >
                      {t("connection.dialog.backToType")}
                    </Button>
                  ) : null}
                </div>

                <div className="grid gap-2">
                  <Label htmlFor="name">
                    {t("connection.dialog.fields.connectionName")}
                  </Label>
                  <Input
                    id="name"
                    value={form.name || ""}
                    onChange={(e) =>
                      setForm((current) => ({
                        ...current,
                        name: e.target.value,
                      }))
                    }
                  />
                </div>

                {!isFileBased && (
                  <>
                    {(formCapabilities.showHost ||
                      formCapabilities.showPort) && (
                      <div className="grid gap-2 sm:grid-cols-2">
                        {formCapabilities.showHost ? (
                          <div className="grid gap-2">
                            <Label htmlFor="host">
                              {t("connection.dialog.fields.host")}{" "}
                              <span className="text-red-600">*</span>
                            </Label>
                            <Input
                              id="host"
                              placeholder={
                                form.driver === "redis"
                                  ? "127.0.0.1 or 10.0.0.1:6379,10.0.0.2:6379"
                                  : undefined
                              }
                              value={form.host || ""}
                              onChange={(e) =>
                                setForm((current) => ({
                                  ...current,
                                  host: e.target.value,
                                }))
                              }
                            />
                          </div>
                        ) : null}
                        {formCapabilities.showPort ? (
                          <div className="grid gap-2">
                            <Label htmlFor="port">
                              {t("connection.dialog.fields.port")}{" "}
                              <span className="text-red-600">*</span>
                            </Label>
                            <Input
                              id="port"
                              placeholder={String(
                                getDefaultPort(form.driver) ?? "",
                              )}
                              value={String(form.port || "")}
                              onChange={(e) =>
                                setForm((current) => ({
                                  ...current,
                                  port: Number(e.target.value) || undefined,
                                }))
                              }
                            />
                          </div>
                        ) : null}
                      </div>
                    )}

                    {(formCapabilities.showUsername ||
                      formCapabilities.showPassword) && (
                      <div className="grid gap-2 sm:grid-cols-2">
                        {formCapabilities.showUsername ? (
                          <div className="grid gap-2">
                            <Label htmlFor="username">
                              {t("connection.dialog.fields.username")}{" "}
                              {isUsernameRequired ? (
                                <span className="text-red-600">*</span>
                              ) : null}
                            </Label>
                            <Input
                              id="username"
                              value={form.username || ""}
                              onChange={(e) =>
                                setForm((current) => ({
                                  ...current,
                                  username: e.target.value,
                                }))
                              }
                            />
                          </div>
                        ) : null}
                        {formCapabilities.showPassword ? (
                          <div className="grid gap-2">
                            <Label htmlFor="password">
                              {t("connection.dialog.fields.password")}{" "}
                              {dialogMode === "create" &&
                              isPasswordRequiredOnCreate ? (
                                <span className="text-red-600">*</span>
                              ) : null}
                            </Label>
                            <Input
                              id="password"
                              type="password"
                              placeholder={
                                dialogMode === "edit"
                                  ? t(
                                      "connection.dialog.placeholders.keepPassword",
                                    )
                                  : undefined
                              }
                              value={form.password || ""}
                              onChange={(e) =>
                                setForm((current) => ({
                                  ...current,
                                  password: e.target.value,
                                }))
                              }
                            />
                          </div>
                        ) : null}
                      </div>
                    )}

                    {(formCapabilities.showDatabase ||
                      formCapabilities.showSchema) && (
                      <div className="grid gap-2 sm:grid-cols-2">
                        {formCapabilities.showDatabase ? (
                          <div className="grid gap-2">
                            <Label htmlFor="database">
                              {t("connection.dialog.fields.database")}
                            </Label>
                            <Input
                              id="database"
                              value={form.database || ""}
                              onChange={(e) =>
                                setForm((current) => ({
                                  ...current,
                                  database: e.target.value,
                                }))
                              }
                            />
                          </div>
                        ) : null}
                        {formCapabilities.showSchema ? (
                          <div className="grid gap-2">
                            <Label htmlFor="schema">
                              {t("connection.dialog.fields.schema")}
                            </Label>
                            <Input
                              id="schema"
                              value={form.schema || ""}
                              onChange={(e) =>
                                setForm((current) => ({
                                  ...current,
                                  schema: e.target.value,
                                }))
                              }
                            />
                          </div>
                        ) : null}
                      </div>
                    )}

                    {formCapabilities.showSsl ? (
                      <>
                        <div className="flex items-center space-x-2">
                          <Checkbox
                            id="ssl"
                            checked={form.ssl}
                            onCheckedChange={(checked) =>
                              setForm((current) => ({
                                ...current,
                                ssl: checked === true,
                              }))
                            }
                          />
                          <Label htmlFor="ssl">
                            {t("connection.dialog.fields.ssl")}
                          </Label>
                        </div>
                        {form.ssl && supportsSslCa ? (
                          <div className="space-y-3 rounded-md border bg-muted/20 p-3">
                            <div className="grid gap-2">
                              <Label htmlFor="sslMode">
                                {t("connection.dialog.fields.sslMode")}
                              </Label>
                              <Select
                                value={form.sslMode || "require"}
                                onValueChange={(
                                  value: "require" | "verify_ca",
                                ) =>
                                  setForm((current) => ({
                                    ...current,
                                    sslMode: value,
                                  }))
                                }
                              >
                                <SelectTrigger id="sslMode">
                                  <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                  <SelectItem value="require">
                                    {t("connection.dialog.sslMode.require")}
                                  </SelectItem>
                                  <SelectItem value="verify_ca">
                                    {t("connection.dialog.sslMode.verifyCa")}
                                  </SelectItem>
                                </SelectContent>
                              </Select>
                            </div>
                            {form.sslMode === "verify_ca" ? (
                              <div className="grid gap-2">
                                <Label htmlFor="sslCaCert">
                                  {t("connection.dialog.fields.sslCaCert")}{" "}
                                  <span className="text-red-600">*</span>
                                </Label>
                                <div className="flex justify-end">
                                  <Button
                                    type="button"
                                    variant="outline"
                                    size="sm"
                                    onClick={onPickSslCaCertFile}
                                  >
                                    <FolderOpen className="mr-2 h-4 w-4" />
                                    {t("connection.dialog.browse")}
                                  </Button>
                                </div>
                                <Textarea
                                  id="sslCaCert"
                                  rows={5}
                                  placeholder={t(
                                    "connection.dialog.placeholders.sslCaCert",
                                  )}
                                  value={form.sslCaCert || ""}
                                  onChange={(e) =>
                                    setForm((current) => ({
                                      ...current,
                                      sslCaCert: e.target.value,
                                    }))
                                  }
                                />
                              </div>
                            ) : null}
                          </div>
                        ) : null}
                      </>
                    ) : null}

                    {formCapabilities.showSsh ? (
                      <>
                        <div className="flex items-center space-x-2">
                          <Checkbox
                            id="ssh"
                            checked={form.sshEnabled}
                            onCheckedChange={(checked) =>
                              setForm((current) => ({
                                ...current,
                                sshEnabled: checked === true,
                              }))
                            }
                          />
                          <Label htmlFor="ssh">
                            {t("connection.dialog.fields.ssh")}
                          </Label>
                        </div>
                        {form.sshEnabled ? (
                          <div className="space-y-3 rounded-md border bg-muted/20 p-3">
                            <div className="grid gap-2 sm:grid-cols-2">
                              <div className="grid gap-2">
                                <Label htmlFor="sshHost">
                                  {t("connection.dialog.fields.sshHost")}
                                </Label>
                                <Input
                                  id="sshHost"
                                  placeholder={t(
                                    "connection.dialog.placeholders.sshHost",
                                  )}
                                  value={form.sshHost || ""}
                                  onChange={(e) =>
                                    setForm((current) => ({
                                      ...current,
                                      sshHost: e.target.value,
                                    }))
                                  }
                                />
                              </div>
                              <div className="grid gap-2">
                                <Label htmlFor="sshPort">
                                  {t("connection.dialog.fields.sshPort")}
                                </Label>
                                <Input
                                  id="sshPort"
                                  placeholder={t(
                                    "connection.dialog.placeholders.sshPort",
                                  )}
                                  value={String(form.sshPort || "")}
                                  onChange={(e) =>
                                    setForm((current) => ({
                                      ...current,
                                      sshPort:
                                        Number(e.target.value) || undefined,
                                    }))
                                  }
                                />
                              </div>
                            </div>
                            <div className="grid gap-2">
                              <Label htmlFor="sshUsername">
                                {t("connection.dialog.fields.sshUsername")}
                              </Label>
                              <Input
                                id="sshUsername"
                                placeholder={t(
                                  "connection.dialog.placeholders.sshUsername",
                                )}
                                value={form.sshUsername || ""}
                                onChange={(e) =>
                                  setForm((current) => ({
                                    ...current,
                                    sshUsername: e.target.value,
                                  }))
                                }
                              />
                            </div>
                            <div className="grid gap-2">
                              <Label htmlFor="sshPassword">
                                {t("connection.dialog.fields.sshPassword")}
                              </Label>
                              <Input
                                id="sshPassword"
                                type="password"
                                placeholder={t(
                                  "connection.dialog.placeholders.sshPassword",
                                )}
                                value={form.sshPassword || ""}
                                onChange={(e) =>
                                  setForm((current) => ({
                                    ...current,
                                    sshPassword: e.target.value,
                                  }))
                                }
                              />
                            </div>
                            <div className="grid gap-2">
                              <Label htmlFor="sshKeyPath">
                                {t("connection.dialog.fields.sshKeyPath")}
                              </Label>
                              <div className="flex gap-2">
                                <Input
                                  id="sshKeyPath"
                                  placeholder={t(
                                    "connection.dialog.placeholders.sshKeyPath",
                                  )}
                                  value={form.sshKeyPath || ""}
                                  onChange={(e) =>
                                    setForm((current) => ({
                                      ...current,
                                      sshKeyPath: e.target.value,
                                    }))
                                  }
                                />
                                <Button
                                  type="button"
                                  variant="outline"
                                  onClick={onPickSshKeyFile}
                                >
                                  <FolderOpen className="mr-2 h-4 w-4" />
                                  {t("connection.dialog.browse")}
                                </Button>
                              </div>
                            </div>
                          </div>
                        ) : null}
                      </>
                    ) : null}
                  </>
                )}

                {formCapabilities.showFilePath ? (
                  <div className="grid gap-2">
                    <Label htmlFor="filePath">
                      {form.driver === "duckdb"
                        ? t("connection.dialog.fields.duckdbFilePath")
                        : t("connection.dialog.fields.sqliteFilePath")}{" "}
                      <span className="text-red-600">*</span>
                    </Label>
                    <div className="flex gap-2">
                      <Input
                        id="filePath"
                        placeholder={
                          form.driver === "duckdb"
                            ? t("connection.dialog.placeholders.duckdbPath")
                            : t("connection.dialog.placeholders.sqlitePath")
                        }
                        value={form.filePath || ""}
                        onChange={(e) =>
                          setForm((current) => ({
                            ...current,
                            filePath: e.target.value,
                          }))
                        }
                        className="flex-1"
                      />
                      <Button
                        type="button"
                        variant="outline"
                        onClick={() => onPickDatabaseFile(form.driver)}
                      >
                        <FolderOpen className="mr-2 h-4 w-4" />
                        {t("connection.dialog.browse")}
                      </Button>
                    </div>
                  </div>
                ) : null}

                {formCapabilities.showSqliteKey ? (
                  <div className="grid gap-2">
                    <Label htmlFor="sqliteKey">
                      {t("connection.dialog.fields.sqliteKey")}
                    </Label>
                    <Input
                      id="sqliteKey"
                      type="password"
                      placeholder={t(
                        "connection.dialog.placeholders.sqliteKey",
                      )}
                      value={form.password || ""}
                      onChange={(e) =>
                        setForm((current) => ({
                          ...current,
                          password: e.target.value,
                        }))
                      }
                    />
                  </div>
                ) : null}
              </>
            )}
          </div>

          {isCreateTypeStep ? (
            <div className="flex justify-end gap-2">
              <Button type="button" variant="outline" onClick={onClose}>
                {t("common.cancel")}
              </Button>
            </div>
          ) : (
            <div className="flex justify-end gap-2">
              <Button type="button" variant="outline" onClick={onClose}>
                {t("common.cancel")}
              </Button>
              <Button
                type="button"
                variant="outline"
                onClick={onTestConnection}
                disabled={isTesting}
              >
                {isTesting ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    {t("connection.dialog.testing")}
                  </>
                ) : (
                  t("connection.dialog.test")
                )}
              </Button>
              <Button
                type="submit"
                disabled={
                  (dialogMode === "edit" ? isSavingEdit : isConnecting) ||
                  !requiredOk
                }
              >
                {dialogMode === "edit" ? (
                  isSavingEdit ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      {t("connection.dialog.saving")}
                    </>
                  ) : (
                    t("common.save")
                  )
                ) : isConnecting ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    {t("connection.dialog.connecting")}
                  </>
                ) : (
                  t("connection.dialog.connect")
                )}
              </Button>
            </div>
          )}

          {validationMsg ? (
            <div className="mt-3">
              <Alert variant="destructive">
                <AlertTitle>
                  {t("connection.dialog.validationFailed")}
                </AlertTitle>
                <AlertDescription>{validationMsg}</AlertDescription>
              </Alert>
            </div>
          ) : null}
          {testMsg && !isCreateTypeStep ? (
            <div className="mt-3">
              <Alert variant={testMsg.ok ? "default" : "destructive"}>
                <AlertTitle>
                  {testMsg.ok
                    ? t("connection.dialog.testSuccess")
                    : t("connection.dialog.testFailed")}
                </AlertTitle>
                <AlertDescription>
                  {testMsg.text}
                  {testMsg.latency ? `(${testMsg.latency}ms)` : ""}
                </AlertDescription>
              </Alert>
            </div>
          ) : null}
        </form>
      </DialogContent>
    </Dialog>
  );
}
