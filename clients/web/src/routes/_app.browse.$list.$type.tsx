import { DiscoverList, TmdbKind } from '@kromatv/client/discovery';
import { createFileRoute, redirect } from '@tanstack/react-router';
import { DiscoverListPage } from '#web/features/requests/discover-list-page';

export const Route = createFileRoute('/_app/browse/$list/$type')({
  beforeLoad: ({ params }) => {
    if (!DiscoverList.safeParse(params.list).success || !TmdbKind.safeParse(params.type).success) {
      throw redirect({ to: '/search', search: { q: '', type: 'all' } });
    }
  },
  component: BrowseRoute,
});

function BrowseRoute() {
  const { list, type } = Route.useParams();
  return (
    <DiscoverListPage
      key={`${list}-${type}`}
      list={DiscoverList.parse(list)}
      type={TmdbKind.parse(type)}
    />
  );
}
