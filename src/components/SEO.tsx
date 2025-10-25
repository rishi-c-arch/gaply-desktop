import { useEffect } from 'react';
import { useLocation } from 'react-router-dom';

interface SEOProps {
  title?: string;
  description?: string;
  keywords?: string;
  canonical?: string;
}

const SEO: React.FC<SEOProps> = ({ 
  title = "Gaply - AI-Powered Academic Research Platform | Thesis Writing, Journal Matching & Research Support",
  description = "Professional academic research platform offering AI content detection, thesis writing help, journal matching, research paper editing, and statistical analysis support for PhD students and researchers worldwide.",
  keywords = "academic research platform, thesis writing help, AI content detector, journal matching service, research paper editing, dissertation writing assistance, PhD thesis support, academic proofreading, plagiarism checker, statistical analysis help, Scopus journal finder, research methodology guidance, academic writing service, literature review help, conference paper preparation, research proposal writing, data analysis support, academic consultation",
  canonical
}) => {
  const location = useLocation();

  useEffect(() => {
    // Update document title
    document.title = title;

    // Update meta description
    const metaDescription = document.querySelector('meta[name="description"]');
    if (metaDescription) {
      metaDescription.setAttribute('content', description);
    }

    // Update meta keywords
    const metaKeywords = document.querySelector('meta[name="keywords"]');
    if (metaKeywords) {
      metaKeywords.setAttribute('content', keywords);
    }

    // Update canonical URL
    const canonicalLink = document.querySelector('link[rel="canonical"]');
    if (canonicalLink) {
      canonicalLink.setAttribute('href', canonical || `https://www.gaply.in${location.pathname}`);
    }

    // Update Open Graph tags
    const ogTitle = document.querySelector('meta[property="og:title"]');
    if (ogTitle) {
      ogTitle.setAttribute('content', title);
    }

    const ogDescription = document.querySelector('meta[property="og:description"]');
    if (ogDescription) {
      ogDescription.setAttribute('content', description);
    }

    const ogUrl = document.querySelector('meta[property="og:url"]');
    if (ogUrl) {
      ogUrl.setAttribute('content', `https://www.gaply.in${location.pathname}`);
    }

    // Update Twitter tags
    const twitterTitle = document.querySelector('meta[property="twitter:title"]');
    if (twitterTitle) {
      twitterTitle.setAttribute('content', title);
    }

    const twitterDescription = document.querySelector('meta[property="twitter:description"]');
    if (twitterDescription) {
      twitterDescription.setAttribute('content', description);
    }

    const twitterUrl = document.querySelector('meta[property="twitter:url"]');
    if (twitterUrl) {
      twitterUrl.setAttribute('content', `https://www.gaply.in${location.pathname}`);
    }

  }, [title, description, keywords, canonical, location.pathname]);

  return null; // This component doesn't render anything
};

export default SEO;
