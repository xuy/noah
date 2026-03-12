import { useState, useEffect } from "react";
import {
  View,
  Text,
  TextInput,
  TouchableOpacity,
  StyleSheet,
  Alert,
  KeyboardAvoidingView,
  Platform,
  ScrollView,
} from "react-native";
import * as SecureStore from "expo-secure-store";
import { SafeAreaView } from "react-native-safe-area-context";
import { colors } from "../../constants/theme";

const API_KEY_STORAGE_KEY = "noah_api_key";

export default function SettingsTab() {
  const [apiKey, setApiKey] = useState("");
  const [hasStoredKey, setHasStoredKey] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    loadStoredKey();
  }, []);

  async function loadStoredKey() {
    try {
      const stored = await SecureStore.getItemAsync(API_KEY_STORAGE_KEY);
      if (stored) {
        setHasStoredKey(true);
        // Show masked version of the stored key
        setApiKey(maskKey(stored));
      }
    } catch {
      // SecureStore not available (e.g. web)
    } finally {
      setIsLoading(false);
    }
  }

  function maskKey(key: string): string {
    if (key.length <= 8) return "****";
    return key.slice(0, 4) + "****" + key.slice(-4);
  }

  async function handleSave() {
    const trimmed = apiKey.trim();
    if (!trimmed) {
      Alert.alert("Empty Key", "Please enter an API key before saving.");
      return;
    }
    // Don't save the masked version
    if (hasStoredKey && trimmed === maskKey(trimmed)) {
      Alert.alert("No Changes", "The API key has not been modified.");
      return;
    }
    try {
      await SecureStore.setItemAsync(API_KEY_STORAGE_KEY, trimmed);
      setHasStoredKey(true);
      setApiKey(maskKey(trimmed));
      Alert.alert("Saved", "API key stored securely.");
    } catch {
      Alert.alert("Error", "Failed to save API key. Secure storage may not be available on this device.");
    }
  }

  async function handleClear() {
    Alert.alert(
      "Clear API Key",
      "Are you sure you want to remove the stored API key?",
      [
        { text: "Cancel", style: "cancel" },
        {
          text: "Clear",
          style: "destructive",
          onPress: async () => {
            try {
              await SecureStore.deleteItemAsync(API_KEY_STORAGE_KEY);
              setApiKey("");
              setHasStoredKey(false);
              Alert.alert("Cleared", "API key has been removed.");
            } catch {
              Alert.alert("Error", "Failed to clear API key.");
            }
          },
        },
      ]
    );
  }

  if (isLoading) {
    return (
      <SafeAreaView style={styles.container} edges={["bottom"]}>
        <View style={styles.centered}>
          <Text style={styles.loadingText}>Loading...</Text>
        </View>
      </SafeAreaView>
    );
  }

  return (
    <SafeAreaView style={styles.container} edges={["bottom"]}>
      <KeyboardAvoidingView
        behavior={Platform.OS === "ios" ? "padding" : "height"}
        style={styles.flex}
      >
        <ScrollView
          contentContainerStyle={styles.scrollContent}
          keyboardShouldPersistTaps="handled"
        >
          <View style={styles.section}>
            <Text style={styles.sectionTitle}>API Configuration</Text>
            <Text style={styles.sectionDescription}>
              Enter your API key to enable photo analysis. The key is stored
              securely on your device and never transmitted to third parties.
            </Text>

            <View style={styles.inputGroup}>
              <Text style={styles.label}>API Key</Text>
              <TextInput
                style={styles.input}
                value={apiKey}
                onChangeText={(text) => {
                  setApiKey(text);
                }}
                onFocus={() => {
                  // Clear masked value when user starts editing
                  if (hasStoredKey) {
                    setApiKey("");
                  }
                }}
                placeholder="sk-..."
                placeholderTextColor={colors.textMuted}
                secureTextEntry={!hasStoredKey}
                autoCapitalize="none"
                autoCorrect={false}
                autoComplete="off"
              />
            </View>

            <View style={styles.buttonRow}>
              <TouchableOpacity style={styles.primaryButton} onPress={handleSave}>
                <Text style={styles.primaryButtonText}>Save</Text>
              </TouchableOpacity>

              {hasStoredKey && (
                <TouchableOpacity
                  style={styles.dangerButton}
                  onPress={handleClear}
                >
                  <Text style={styles.dangerButtonText}>Clear</Text>
                </TouchableOpacity>
              )}
            </View>

            {hasStoredKey && (
              <View style={styles.statusRow}>
                <View style={styles.statusDot} />
                <Text style={styles.statusText}>API key is stored</Text>
              </View>
            )}
          </View>

          <View style={styles.footer}>
            <Text style={styles.footerText}>Noah v1.0.0</Text>
          </View>
        </ScrollView>
      </KeyboardAvoidingView>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: colors.bgPrimary,
  },
  flex: {
    flex: 1,
  },
  centered: {
    flex: 1,
    justifyContent: "center",
    alignItems: "center",
  },
  loadingText: {
    color: colors.textSecondary,
    fontSize: 16,
  },
  scrollContent: {
    padding: 20,
    flexGrow: 1,
  },
  section: {
    backgroundColor: colors.bgSecondary,
    borderRadius: 16,
    padding: 20,
    borderWidth: 1,
    borderColor: colors.borderPrimary,
  },
  sectionTitle: {
    fontSize: 20,
    fontWeight: "700",
    color: colors.textPrimary,
    marginBottom: 8,
  },
  sectionDescription: {
    fontSize: 14,
    color: colors.textSecondary,
    lineHeight: 22,
    marginBottom: 24,
  },
  inputGroup: {
    marginBottom: 20,
  },
  label: {
    fontSize: 14,
    fontWeight: "600",
    color: colors.textSecondary,
    marginBottom: 8,
  },
  input: {
    backgroundColor: colors.bgInput,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: colors.borderPrimary,
    paddingHorizontal: 16,
    paddingVertical: 14,
    fontSize: 16,
    color: colors.textPrimary,
  },
  buttonRow: {
    flexDirection: "row",
    gap: 12,
  },
  primaryButton: {
    backgroundColor: colors.accentTeal,
    paddingHorizontal: 28,
    paddingVertical: 14,
    borderRadius: 12,
    flex: 1,
    alignItems: "center",
  },
  primaryButtonText: {
    color: colors.bgPrimary,
    fontSize: 16,
    fontWeight: "600",
  },
  dangerButton: {
    backgroundColor: "transparent",
    paddingHorizontal: 28,
    paddingVertical: 14,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: colors.accentRed,
    flex: 1,
    alignItems: "center",
  },
  dangerButtonText: {
    color: colors.accentRed,
    fontSize: 16,
    fontWeight: "600",
  },
  statusRow: {
    flexDirection: "row",
    alignItems: "center",
    marginTop: 16,
    gap: 8,
  },
  statusDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
    backgroundColor: colors.statusActive,
  },
  statusText: {
    color: colors.statusActive,
    fontSize: 14,
    fontWeight: "500",
  },
  footer: {
    marginTop: "auto",
    paddingTop: 32,
    alignItems: "center",
  },
  footerText: {
    color: colors.textMuted,
    fontSize: 13,
  },
});
