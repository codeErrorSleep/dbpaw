import type { Dispatch, FormEvent, ReactNode, SetStateAction } from "react";
import { useTranslation } from "react-i18next";
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
  DRIVER_REGISTRY,
  supportsSSLCA,
} from "@/lib/driver-registry";
import {
  getConnectionFormCapabilities,
  isFileBasedDriver,
} from "@/lib/connection-form/rules";
import { useFormField } from "@/lib/connection-form/use-form-field";
import type { ConnectionForm, Driver } from "@/services/api";
import { ConnectionBasicFields } from "./ConnectionBasicFields";
import { ConnectionDialogFooter, type ConnectionDialogTestMessage } from "./ConnectionDialogFooter";
import { ConnectionProxyFields } from "./ConnectionProxyFields";
import { ConnectionNetworkFields } from "./ConnectionNetworkFields";
import { ConnectionSecurityFields } from "./ConnectionSecurityFields";
import { ConnectionSummaryHeader } from "./ConnectionSummaryHeader";
import { ConnectionTypeStep } from "./ConnectionTypeStep";

export type { ConnectionDialogTestMessage };

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
  const isCreateTypeStep = dialogMode === "create" && createStep === "type";
  const isRedis = form.driver === "redis";
  const isElasticsearch = form.driver === "elasticsearch";
  const isMssql = form.driver === "mssql";
  const hasElasticCloudId = isElasticsearch && !!(form.cloudId || "").trim();

  const [name, onNameChange] = useFormField(form, setForm, "name");
  const [host, onHostChange] = useFormField(form, setForm, "host");
  const [port, onPortChange] = useFormField(form, setForm, "port", (v) => Number(v) || undefined);
  const [username, onUsernameChange] = useFormField(form, setForm, "username");
  const [password, onPasswordChange] = useFormField(form, setForm, "password");
  const [database, onDatabaseChange] = useFormField(form, setForm, "database");
  const [schema, onSchemaChange] = useFormField(form, setForm, "schema");
  const [sslCaCert, onSslCaCertChange] = useFormField(form, setForm, "sslCaCert");
  const [sshHost, onSshHostChange] = useFormField(form, setForm, "sshHost");
  const [sshPort, onSshPortChange] = useFormField(form, setForm, "sshPort", (v) => Number(v) || undefined);
  const [sshUsername, onSshUsernameChange] = useFormField(form, setForm, "sshUsername");
  const [sshPassword, onSshPasswordChange] = useFormField(form, setForm, "sshPassword");
  const [sshKeyPath, onSshKeyPathChange] = useFormField(form, setForm, "sshKeyPath");
  const [filePath, onFilePathChange] = useFormField(form, setForm, "filePath");

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>{trigger}</DialogTrigger>
      <DialogContent className="max-h-[90dvh] overflow-y-auto sm:max-w-2xl">
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
              <ConnectionTypeStep
                selectedDriver={form.driver}
                onSelect={onCreateDriverSelect}
                previewLabel={t("connection.dialog.driverHints.preview")}
              />
            ) : (
              <>
                <ConnectionSummaryHeader
                  driverConfig={driverConfig}
                  dialogMode={dialogMode}
                  onBackToType={onBackToType}
                  backLabel={t("connection.dialog.backToType")}
                />

                <div className="grid gap-2">
                  <Label htmlFor="name">
                    {t("connection.dialog.fields.connectionName")}
                  </Label>
                  <Input id="name" value={name || ""} onChange={onNameChange} />
                </div>

                {!isFileBased && (
                  <>
                    <ConnectionBasicFields
                      form={form}
                      setForm={setForm}
                      dialogMode={dialogMode}
                      database={database || ""}
                      onDatabaseChange={onDatabaseChange}
                      username={username || ""}
                      onUsernameChange={onUsernameChange}
                      password={password || ""}
                      onPasswordChange={onPasswordChange}
                      schema={schema || ""}
                      onSchemaChange={onSchemaChange}
                      showUsername={formCapabilities.showUsername}
                      showPassword={formCapabilities.showPassword}
                      showDatabase={formCapabilities.showDatabase}
                      showSchema={formCapabilities.showSchema}
                      isRedis={isRedis}
                      isElasticsearch={isElasticsearch}
                      isMssql={isMssql}
                    />

                    <ConnectionNetworkFields
                      driver={form.driver}
                      host={host || ""}
                      onHostChange={onHostChange}
                      port={port}
                      onPortChange={onPortChange}
                      filePath={filePath || ""}
                      onFilePathChange={onFilePathChange}
                      showHost={formCapabilities.showHost}
                      showPort={formCapabilities.showPort}
                      showFilePath={false}
                      isMssql={isMssql}
                      hasElasticCloudId={hasElasticCloudId}
                      onPickDatabaseFile={onPickDatabaseFile}
                    />

                    <ConnectionSecurityFields
                      form={form}
                      setForm={setForm}
                      showSsl={formCapabilities.showSsl}
                      showSsh={formCapabilities.showSsh}
                      supportsSslCa={supportsSslCa}
                      hasElasticCloudId={hasElasticCloudId}
                      sslCaCert={sslCaCert || ""}
                      onSslCaCertChange={onSslCaCertChange}
                      sshHost={sshHost || ""}
                      onSshHostChange={onSshHostChange}
                      sshPort={sshPort}
                      onSshPortChange={onSshPortChange}
                      sshUsername={sshUsername || ""}
                      onSshUsernameChange={onSshUsernameChange}
                      sshPassword={sshPassword || ""}
                      onSshPasswordChange={onSshPasswordChange}
                      sshKeyPath={sshKeyPath || ""}
                      onSshKeyPathChange={onSshKeyPathChange}
                      onPickSslCaCertFile={onPickSslCaCertFile}
                      onPickSshKeyFile={onPickSshKeyFile}
                    />
                  </>
                )}

                <ConnectionProxyFields form={form} setForm={setForm} />

                <ConnectionNetworkFields
                  driver={form.driver}
                  host={host || ""}
                  onHostChange={onHostChange}
                  port={port}
                  onPortChange={onPortChange}
                  filePath={filePath || ""}
                  onFilePathChange={onFilePathChange}
                  showHost={false}
                  showPort={false}
                  showFilePath={formCapabilities.showFilePath}
                  isMssql={isMssql}
                  hasElasticCloudId={hasElasticCloudId}
                  onPickDatabaseFile={onPickDatabaseFile}
                />

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
                      value={password || ""}
                      onChange={onPasswordChange}
                    />
                  </div>
                ) : null}
              </>
            )}
          </div>

          <ConnectionDialogFooter
            dialogMode={dialogMode}
            isCreateTypeStep={isCreateTypeStep}
            requiredOk={requiredOk}
            isTesting={isTesting}
            isConnecting={isConnecting}
            isSavingEdit={isSavingEdit}
            validationMsg={validationMsg}
            testMsg={testMsg}
            onClose={onClose}
            onTestConnection={onTestConnection}
          />
        </form>
      </DialogContent>
    </Dialog>
  );
}
