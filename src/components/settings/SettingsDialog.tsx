import { Palette, Info } from "lucide-react";
import { useTheme, Theme } from "@/components/theme-provider";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const THEME_COLORS = [
  { name: "Zinc", value: "#09090b" },
  { name: "Blue", value: "#3b82f6" },
  { name: "Violet", value: "#8b5cf6" },
  { name: "Green", value: "#22c55e" },
  { name: "Orange", value: "#f97316" },
];

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const { theme, setTheme, accentColor, setAccentColor } = useTheme();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>
            Manage your app appearance and preferences.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-6 py-4">
          {/* Appearance Section */}
          <div className="space-y-4">
            <h3 className="text-lg font-medium flex items-center gap-2">
              <Palette className="w-5 h-5" /> Appearance
            </h3>

            <div className="grid grid-cols-2 gap-4 items-center">
              <div className="space-y-1">
                <Label className="text-base">Theme Mode</Label>
                <p className="text-xs text-muted-foreground">
                  Choose your interface style
                </p>
              </div>
              <Select value={theme} onValueChange={(v) => setTheme(v as Theme)}>
                <SelectTrigger>
                  <SelectValue placeholder="Select theme" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="light">☀️ Light Mode</SelectItem>
                  <SelectItem value="dark">🌙 Dark Mode</SelectItem>
                  <SelectItem value="system">🖥️ System</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-3 pt-2">
              <div className="flex items-center justify-between">
                <Label className="text-base">Accent Color</Label>
              </div>
              <div className="flex flex-wrap gap-3">
                {THEME_COLORS.map((color) => (
                  <button
                    key={color.name}
                    className={`h-8 w-8 rounded-full border-2 flex items-center justify-center transition-all ${accentColor === color.name
                      ? "border-primary ring-2 ring-ring ring-offset-2 scale-110"
                      : "border-transparent hover:scale-105"
                      }`}
                    style={{ backgroundColor: color.value }}
                    onClick={() => setAccentColor(color.name)}
                    title={color.name}
                  >
                    {accentColor === color.name && (
                      <div className="w-2 h-2 bg-white rounded-full" />
                    )}
                  </button>
                ))}
              </div>
            </div>
          </div>

          <Separator />

          {/* About Section */}
          <div className="space-y-4">
            <h3 className="text-lg font-medium flex items-center gap-2">
              <Info className="w-5 h-5" /> About
            </h3>
            <div className="bg-muted/50 rounded-lg p-4 space-y-2">
              <div className="flex items-center justify-between">
                <span className="font-medium">DbPaw</span>
                <span className="text-sm text-muted-foreground">v0.1.0</span>
              </div>
              <p className="text-sm text-muted-foreground">
                A modern database management tool providing a smooth development experience.
              </p>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
