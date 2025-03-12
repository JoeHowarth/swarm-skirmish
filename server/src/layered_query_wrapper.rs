fn top(
    query: Query<(Entity, &BotId, &Pos, &Energy, &CurrentAction, &PastActions)>,
) {
    for (entity, bot_id, pos, energy, current_action, past_actions) in
        query.iter()
    {
        let mut hashmap = HashMap::new();
        hashmap.insert(
            entity,
            (entity, bot_id, pos, energy, current_action, past_actions),
        );

        let h = EitherHolder::new(&query);

        let x = h.get_pos(entity);

        let h2 = EitherHolder::<(
            Entity,
            &BotId,
            &Pos,
            &Energy,
            &CurrentAction,
            &PastActions,
        )>::from_map(&hashmap);

        let y = h2.get_pos(entity);

        {
            h2.iter().for_each(|(entity, _, pos, _, _, _)| {
                dbg!(entity, pos);
            });
        }
    }
}

struct LayeredHolder<'a, 'w, 's, D: QueryData>
where
    <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>: Clone,
{
    query: &'a Query<'w, 's, D, ()>,
    hashmap: &'a mut HashMap<
        Entity,
        <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>,
    >,
}

impl<'a: 's, 'w: 's, 's, D: QueryData> LayeredHolder<'a, 'w, 's, D>
where
    <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>: Clone,
{
    fn new(
        query: &'a Query<'w, 's, D, ()>,
        hashmap: &'a mut HashMap<
            Entity,
            <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>,
        >,
    ) -> Self {
        Self { query, hashmap }
    }

    fn get_pos(
        &self,
        entity: Entity,
    ) -> Option<<<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>> {
        self.query
            .get(entity)
            .ok()
            .or_else(|| self.hashmap.get(&entity).cloned())
    }

    fn iter(
        &'s self,
    ) -> impl Iterator<Item = <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>>
    {
        self.query.iter()
    }
}

enum EitherHolder<'a, 'w, 's, D: QueryData>
where
    <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>: Clone,
{
    QueryHolder(&'a Query<'w, 's, D, ()>),
    HashMapHolder(
        &'a HashMap<
            Entity,
            <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>,
        >,
    ),
}

impl<'a: 's, 'w: 's, 's, D: QueryData> EitherHolder<'a, 'w, 's, D>
where
    <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>: Clone,
{
    fn new(query: &'a Query<'w, 's, D, ()>) -> Self {
        Self::QueryHolder(query)
    }

    fn from_map(
        hashmap: &'a HashMap<
            Entity,
            <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>,
        >,
    ) -> Self {
        Self::HashMapHolder(hashmap)
    }

    fn get_pos(
        &self,
        entity: Entity,
    ) -> Option<<<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>> {
        match self {
            Self::QueryHolder(query) => query.get(entity).ok(),
            Self::HashMapHolder(hashmap) => hashmap.get(&entity).cloned(),
        }
    }

    fn iter(
        &'s self,
    ) -> impl Iterator<Item = <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>>
    {
        match self {
            EitherHolder::QueryHolder(query) => {
                EitherHolderIter::<D>::QueryIter(query.iter())
            }
            EitherHolder::HashMapHolder(hash_map) => {
                EitherHolderIter::HashMapIter(hash_map.values())
            }
        }
    }
}

enum EitherHolderIter<'w, 's, D: QueryData>
where
    <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>: Clone,
{
    QueryIter(QueryIter<'s, 'w, <D as QueryData>::ReadOnly, ()>),
    HashMapIter(
        Values<
            's,
            Entity,
            <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>,
        >,
    ),
}

impl<'w: 's, 's, D: QueryData> Iterator for EitherHolderIter<'w, 's, D>
where
    <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>: Clone,
{
    type Item = <<D as QueryData>::ReadOnly as WorldQuery>::Item<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::QueryIter(iter) => iter.next(),
            Self::HashMapIter(iter) => iter.next().cloned(),
        }
    }
}