import { useState } from "react";
import {
  View,
  Text,
  TextInput,
  TouchableOpacity,
  StyleSheet,
  ActivityIndicator,
  KeyboardAvoidingView,
  Platform,
  ScrollView,
} from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import * as Google from "expo-auth-session/providers/google";
import * as WebBrowser from "expo-web-browser";
import { colors } from "../constants/theme";
import { signInWithGoogle, redeemInviteCode, type AuthState } from "../lib/auth";

WebBrowser.maybeCompleteAuthSession();

// Placeholder client IDs — replace with real ones when configured
const GOOGLE_CLIENT_ID_IOS = "placeholder.apps.googleusercontent.com";
const GOOGLE_CLIENT_ID_ANDROID = "placeholder.apps.googleusercontent.com";
const GOOGLE_CLIENT_ID_WEB = "placeholder.apps.googleusercontent.com";

interface AuthScreenProps {
  onAuthenticated: (state: AuthState) => void;
}

export default function AuthScreen({ onAuthenticated }: AuthScreenProps) {
  const [inviteCode, setInviteCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [redeemLoading, setRedeemLoading] = useState(false);

  const [_request, response, promptAsync] = Google.useIdTokenAuthRequest({
    iosClientId: GOOGLE_CLIENT_ID_IOS,
    androidClientId: GOOGLE_CLIENT_ID_ANDROID,
    webClientId: GOOGLE_CLIENT_ID_WEB,
  });

  async function handleGoogleSignIn() {
    setError(null);
    setLoading(true);
    try {
      const result = await promptAsync();
      if (result?.type === "success" && result.params?.id_token) {
        const authState = await signInWithGoogle(result.params.id_token);
        onAuthenticated(authState);
      } else if (result?.type === "cancel") {
        // User cancelled — do nothing
      } else {
        setError("Google sign-in did not complete. Please try again.");
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Sign-in failed.";
      setError(msg);
    } finally {
      setLoading(false);
    }
  }

  function formatInviteCode(text: string): string {
    // Strip everything except alphanumeric
    const clean = text.replace(/[^A-Za-z0-9]/g, "").toUpperCase();
    // Format as NOAH-XXXX-XXXX
    const parts: string[] = [];
    if (clean.length > 0) parts.push(clean.slice(0, 4));
    if (clean.length > 4) parts.push(clean.slice(4, 8));
    if (clean.length > 8) parts.push(clean.slice(8, 12));
    return parts.join("-");
  }

  function handleInviteCodeChange(text: string) {
    setInviteCode(formatInviteCode(text));
  }

  async function handleRedeem() {
    const trimmed = inviteCode.trim();
    if (!trimmed) {
      setError("Please enter an invite code.");
      return;
    }
    setError(null);
    setRedeemLoading(true);
    try {
      const authState = await redeemInviteCode(trimmed);
      onAuthenticated(authState);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Redeem failed.";
      setError(msg);
    } finally {
      setRedeemLoading(false);
    }
  }

  return (
    <SafeAreaView style={styles.container}>
      <KeyboardAvoidingView
        behavior={Platform.OS === "ios" ? "padding" : "height"}
        style={styles.flex}
      >
        <ScrollView
          contentContainerStyle={styles.scrollContent}
          keyboardShouldPersistTaps="handled"
        >
          {/* Branding */}
          <View style={styles.brandingSection}>
            <Text style={styles.logo}>Noah</Text>
            <Text style={styles.tagline}>
              IT support, powered by AI
            </Text>
          </View>

          {/* Error message */}
          {error && (
            <View style={styles.errorCard}>
              <Text style={styles.errorText}>{error}</Text>
            </View>
          )}

          {/* Google Sign In */}
          <TouchableOpacity
            style={styles.googleButton}
            onPress={handleGoogleSignIn}
            disabled={loading}
            activeOpacity={0.8}
          >
            {loading ? (
              <ActivityIndicator color={colors.bgPrimary} size="small" />
            ) : (
              <Text style={styles.googleButtonText}>Sign in with Google</Text>
            )}
          </TouchableOpacity>

          {/* Divider */}
          <View style={styles.divider}>
            <View style={styles.dividerLine} />
            <Text style={styles.dividerText}>or</Text>
            <View style={styles.dividerLine} />
          </View>

          {/* Invite Code */}
          <View style={styles.inviteSection}>
            <Text style={styles.inviteLabel}>Invite Code</Text>
            <TextInput
              style={styles.inviteInput}
              value={inviteCode}
              onChangeText={handleInviteCodeChange}
              placeholder="NOAH-XXXX-XXXX"
              placeholderTextColor={colors.textMuted}
              autoCapitalize="characters"
              autoCorrect={false}
              autoComplete="off"
              maxLength={14}
            />
            <TouchableOpacity
              style={[
                styles.redeemButton,
                !inviteCode.trim() && styles.redeemButtonDisabled,
              ]}
              onPress={handleRedeem}
              disabled={redeemLoading || !inviteCode.trim()}
              activeOpacity={0.8}
            >
              {redeemLoading ? (
                <ActivityIndicator color={colors.bgPrimary} size="small" />
              ) : (
                <Text style={styles.redeemButtonText}>Redeem</Text>
              )}
            </TouchableOpacity>
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
  scrollContent: {
    flexGrow: 1,
    justifyContent: "center",
    paddingHorizontal: 32,
    paddingVertical: 48,
  },
  brandingSection: {
    alignItems: "center",
    marginBottom: 48,
  },
  logo: {
    fontSize: 48,
    fontWeight: "800",
    color: colors.accentTeal,
    letterSpacing: 2,
    marginBottom: 8,
  },
  tagline: {
    fontSize: 16,
    color: colors.textSecondary,
    textAlign: "center",
  },
  errorCard: {
    backgroundColor: "rgba(248, 113, 113, 0.1)",
    borderWidth: 1,
    borderColor: colors.accentRed,
    borderRadius: 12,
    padding: 14,
    marginBottom: 20,
  },
  errorText: {
    color: colors.accentRed,
    fontSize: 14,
    textAlign: "center",
    lineHeight: 20,
  },
  googleButton: {
    backgroundColor: colors.accentTeal,
    paddingVertical: 16,
    borderRadius: 12,
    alignItems: "center",
    minHeight: 52,
    justifyContent: "center",
  },
  googleButtonText: {
    color: colors.bgPrimary,
    fontSize: 17,
    fontWeight: "700",
  },
  divider: {
    flexDirection: "row",
    alignItems: "center",
    marginVertical: 28,
  },
  dividerLine: {
    flex: 1,
    height: 1,
    backgroundColor: colors.borderPrimary,
  },
  dividerText: {
    color: colors.textMuted,
    fontSize: 14,
    marginHorizontal: 16,
  },
  inviteSection: {
    gap: 12,
  },
  inviteLabel: {
    fontSize: 14,
    fontWeight: "600",
    color: colors.textSecondary,
  },
  inviteInput: {
    backgroundColor: colors.bgInput,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: colors.borderPrimary,
    paddingHorizontal: 16,
    paddingVertical: 14,
    fontSize: 18,
    color: colors.textPrimary,
    textAlign: "center",
    letterSpacing: 2,
    fontWeight: "600",
  },
  redeemButton: {
    backgroundColor: colors.accentPurple,
    paddingVertical: 14,
    borderRadius: 12,
    alignItems: "center",
    minHeight: 48,
    justifyContent: "center",
  },
  redeemButtonDisabled: {
    opacity: 0.5,
  },
  redeemButtonText: {
    color: colors.bgPrimary,
    fontSize: 16,
    fontWeight: "700",
  },
});
