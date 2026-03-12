import { Tabs } from "expo-router";
import { Platform } from "react-native";
import { colors } from "../../constants/theme";

export default function TabLayout() {
  return (
    <Tabs
      screenOptions={{
        headerStyle: {
          backgroundColor: colors.bgSecondary,
        },
        headerTintColor: colors.textPrimary,
        headerTitleStyle: {
          fontWeight: "600",
        },
        tabBarStyle: {
          backgroundColor: colors.bgSecondary,
          borderTopColor: colors.borderPrimary,
          borderTopWidth: 1,
          paddingBottom: Platform.OS === "ios" ? 24 : 8,
          paddingTop: 8,
          height: Platform.OS === "ios" ? 88 : 64,
        },
        tabBarActiveTintColor: colors.accentTeal,
        tabBarInactiveTintColor: colors.textMuted,
        tabBarLabelStyle: {
          fontSize: 12,
          fontWeight: "500",
        },
      }}
    >
      <Tabs.Screen
        name="index"
        options={{
          title: "Camera",
          tabBarIcon: ({ color, size }) => (
            <TabIcon name="camera" color={color} size={size} />
          ),
        }}
      />
      <Tabs.Screen
        name="history"
        options={{
          title: "History",
          tabBarIcon: ({ color, size }) => (
            <TabIcon name="history" color={color} size={size} />
          ),
        }}
      />
      <Tabs.Screen
        name="settings"
        options={{
          title: "Settings",
          tabBarIcon: ({ color, size }) => (
            <TabIcon name="settings" color={color} size={size} />
          ),
        }}
      />
    </Tabs>
  );
}

/**
 * Minimal text-based tab icons (no vector icon library needed).
 * Uses unicode symbols for a lightweight solution.
 */
import { Text } from "react-native";

const iconMap: Record<string, string> = {
  camera: "\u{1F4F7}",   // camera emoji
  history: "\u{1F4CB}",  // clipboard emoji
  settings: "\u{2699}",  // gear emoji
};

function TabIcon({
  name,
  color,
  size,
}: {
  name: string;
  color: string;
  size: number;
}) {
  return (
    <Text style={{ fontSize: size - 4, color, textAlign: "center" }}>
      {iconMap[name] ?? "?"}
    </Text>
  );
}
