import { useState, useEffect, createContext, useContext } from "react";
import { Stack } from "expo-router";
import { StatusBar } from "expo-status-bar";
import { View, ActivityIndicator, StyleSheet } from "react-native";
import { colors } from "../constants/theme";
import {
  getAuthState,
  clearAuth,
  type AuthUser,
  type AuthState,
} from "../lib/auth";
import AuthScreen from "./auth";

interface AuthContextValue {
  user: AuthUser | null;
  token: string | null;
  signOut: () => Promise<void>;
}

const AuthContext = createContext<AuthContextValue>({
  user: null,
  token: null,
  signOut: async () => {},
});

export const useAuth = () => useContext(AuthContext);

export default function RootLayout() {
  const [authState, setAuthState] = useState<AuthState | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    getAuthState()
      .then(setAuthState)
      .finally(() => setLoading(false));
  }, []);

  async function handleSignOut() {
    await clearAuth();
    setAuthState({ token: null, user: null });
  }

  function handleAuthenticated(state: AuthState) {
    setAuthState(state);
  }

  if (loading) {
    return (
      <View style={styles.loadingContainer}>
        <StatusBar style="light" />
        <ActivityIndicator color={colors.accentTeal} size="large" />
      </View>
    );
  }

  if (!authState?.token) {
    return (
      <>
        <StatusBar style="light" />
        <AuthScreen onAuthenticated={handleAuthenticated} />
      </>
    );
  }

  return (
    <AuthContext.Provider
      value={{
        user: authState.user,
        token: authState.token,
        signOut: handleSignOut,
      }}
    >
      <StatusBar style="light" />
      <Stack
        screenOptions={{
          headerShown: false,
          contentStyle: { backgroundColor: colors.bgPrimary },
        }}
      />
    </AuthContext.Provider>
  );
}

const styles = StyleSheet.create({
  loadingContainer: {
    flex: 1,
    backgroundColor: colors.bgPrimary,
    justifyContent: "center",
    alignItems: "center",
  },
});
