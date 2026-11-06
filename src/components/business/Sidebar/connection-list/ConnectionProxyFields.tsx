import type { Dispatch, SetStateAction } from "react";
import { useTranslation } from "react-i18next";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  DEFAULT_NETWORK_PROXY,
  supportsNetworkProxy,
} from "@/lib/connection-form/proxy";
import type { ConnectionForm, NetworkProxy } from "@/services/api";

export function ConnectionProxyFields({
  form,
  setForm,
}: {
  form: ConnectionForm;
  setForm: Dispatch<SetStateAction<ConnectionForm>>;
}) {
  const { t } = useTranslation();
  const proxy = form.proxy ?? DEFAULT_NETWORK_PROXY;
  const supported = supportsNetworkProxy(form);
  if (!supported && !proxy.enabled) return null;
  const update = (patch: Partial<NetworkProxy>) =>
    setForm((current) => ({
      ...current,
      proxy: { ...(current.proxy ?? DEFAULT_NETWORK_PROXY), ...patch },
    }));
  const text = (key: string) => t(`connection.dialog.proxy.${key}`);

  return (
    <div className="grid gap-3 rounded-md border p-3">
      <div className="flex items-center gap-2">
        <Checkbox
          id="proxy-enabled"
          checked={proxy.enabled}
          onCheckedChange={(checked) => update({ enabled: checked === true })}
        />
        <Label htmlFor="proxy-enabled">{text("enabled")}</Label>
      </div>
      <p className="text-xs text-muted-foreground">
        {text(supported ? "description" : "unsupported")}
      </p>
      {proxy.enabled && (
        <>
          <div className="grid gap-2">
            <Label htmlFor="proxy-kind">{text("kind")}</Label>
            <Select
              value={proxy.kind}
              onValueChange={(kind: NetworkProxy["kind"]) => update({ kind })}
            >
              <SelectTrigger id="proxy-kind">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="socks5">SOCKS5</SelectItem>
                <SelectItem value="http">HTTP CONNECT</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="grid gap-2">
              <Label htmlFor="proxy-host">{text("host")}</Label>
              <Input
                id="proxy-host"
                value={proxy.host}
                onChange={(e) => update({ host: e.target.value })}
                placeholder="127.0.0.1"
                autoComplete="off"
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="proxy-port">{text("port")}</Label>
              <Input
                id="proxy-port"
                type="number"
                min={1}
                max={65535}
                value={
                  Number.isFinite(proxy.port) && proxy.port !== 0
                    ? proxy.port
                    : ""
                }
                onChange={(e) => update({ port: Number(e.target.value) })}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="proxy-username">{text("username")}</Label>
              <Input
                id="proxy-username"
                value={proxy.username}
                onChange={(e) => update({ username: e.target.value })}
                autoComplete="off"
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="proxy-password">{text("password")}</Label>
              <Input
                id="proxy-password"
                type="password"
                value={proxy.password}
                onChange={(e) => update({ password: e.target.value })}
                autoComplete="new-password"
              />
            </div>
          </div>
          <p className="text-xs text-muted-foreground">
            {text("passwordHint")}
          </p>
        </>
      )}
    </div>
  );
}
