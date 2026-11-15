import { useKeepAwake } from 'expo-keep-awake';
import { Redirect, useLocalSearchParams, useRouter } from 'expo-router';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { clipRoute } from '#mobile/lib/clip';
import { goBack } from '#mobile/lib/nav';
import { ClipPlayer } from '#mobile/player/clip-player';

export default function ClipRoute() {
  const route =
    clipRoute(useLocalSearchParams<{ uri?: string; title?: string; subtitle?: string }>());
  const router = useRouter();
  useKeepAwake();
  if (!route) return <Redirect href="/" />;
  return (
    <SafeAreaProvider>
      <ClipPlayer key={route.uri} {...route} onClose={() => goBack(router)} />
    </SafeAreaProvider>
  );
}
