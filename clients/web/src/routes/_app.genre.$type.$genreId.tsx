import { TmdbKind } from '@kromatv/client/discovery';
import { createFileRoute, redirect } from '@tanstack/react-router';
import { GenrePage } from '#web/features/requests/discover-list-page';

export const Route = createFileRoute('/_app/genre/$type/$genreId')({
  beforeLoad: ({ params }) => {
    if (!TmdbKind.safeParse(params.type).success || !/^\d+$/.test(params.genreId)) {
      throw redirect({ to: '/search', search: { q: '', type: 'all' } });
    }
  },
  component: GenreRoute,
});

function GenreRoute() {
  const { type, genreId } = Route.useParams();
  return (
    <GenrePage key={`${type}-${genreId}`} genreId={Number(genreId)} type={TmdbKind.parse(type)} />
  );
}
