import React, { useEffect } from 'react';
import { useParams, Link } from 'react-router-dom';
import { BLOG_POSTS } from '../data/blogPosts';
import SEOHead from '../components/SEOHead';
import { sanitizeHtml } from '../utils/sanitizeHtml';
import './BlogPostPage.css';

const BlogPostPage: React.FC = () => {
  const { slug } = useParams<{ slug: string }>();
  const post = BLOG_POSTS.find((p) => p.slug === slug);

  useEffect(() => {
    if (post) {
      document.title = `${post.metaTitle} | Gaply Blog`;
    }
  }, [post]);

  if (!post) {
    return (
      <div className="blog-post-page">
        <div className="blog-post-page__error">
          <h1>Blog post not found</h1>
          <Link to="/blog">← Back to Blog</Link>
        </div>
      </div>
    );
  }

  return (
    <div className="blog-post-page">
      <SEOHead
        title={`${post.metaTitle} | Gaply`}
        description={post.metaDescription}
        keywords={post.keywords.join(', ')}
      />
      <div className="blog-post-page__watermark" aria-hidden="true">
        gaply.in
      </div>
      <article className="blog-post-page__article glass-card">
        <Link to="/blog" className="blog-post-page__back">← Back to Blog</Link>
        <header className="blog-post-page__header">
          <h1 className="blog-post-page__title">{post.title}</h1>
          <div className="blog-post-page__meta">
            <span>{post.author}</span>
            <span>
              {new Date(post.date).toLocaleDateString('en-US', {
                month: 'long',
                day: 'numeric',
                year: 'numeric',
              })}
            </span>
            <span>{post.readTime}</span>
          </div>
        </header>
        {post.imageUrl && (
          <div
            className="blog-post-page__hero"
            style={{ backgroundImage: `url(${post.imageUrl})` }}
          />
        )}
        <div className="blog-post-page__intro">{post.intro}</div>
        <div
          className="blog-post-page__content"
          dangerouslySetInnerHTML={{ __html: sanitizeHtml(post.content) }}
        />
        <footer className="blog-post-page__footer">
          <p><strong>{post.author}</strong> — {post.authorBio}</p>
        </footer>
      </article>
    </div>
  );
};

export default BlogPostPage;
